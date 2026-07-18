'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { emitTo, listen, UnlistenFn } from '@tauri-apps/api/event';
import {
  availableMonitors,
  cursorPosition,
  getCurrentWindow,
  PhysicalPosition,
  Window as TauriWindow,
} from '@tauri-apps/api/window';
import { Mic, X } from 'lucide-react';
import {
  MEETING_DETECTION_START_REQUEST,
  MEETING_DETECTION_START_RESULT,
  MeetingDetectionStartResult,
  recordingStartSucceeded,
  selectMonitorForPoint,
} from '@/hooks/meetingDetectionOverlayProtocol';

interface ConferenceCallDetected {
  session_id: string;
  app_name: string;
  confidence: 'High' | 'Low';
}

interface ConferenceCallEnded {
  session_id: string;
}

const OVERLAY_WIDTH = 440;
const TOP_MARGIN = 28;
const START_TIMEOUT_MS = 45_000;

async function createRecordingResultWaiter(requestId: string) {
  let resolveResult!: (result: MeetingDetectionStartResult) => void;
  let rejectResult!: (error: Error) => void;
  const promise = new Promise<MeetingDetectionStartResult>((resolve, reject) => {
    resolveResult = resolve;
    rejectResult = reject;
  });

  const unlisten = await listen<MeetingDetectionStartResult>(
    MEETING_DETECTION_START_RESULT,
    ({ payload }) => {
      if (payload.request_id === requestId) {
        resolveResult(payload);
      }
    },
  );
  const timeout = window.setTimeout(() => {
    rejectResult(new Error('Meetily did not confirm that recording started.'));
  }, START_TIMEOUT_MS);

  return {
    promise,
    cleanup() {
      window.clearTimeout(timeout);
      unlisten();
    },
  };
}

async function positionOnActiveMonitor() {
  const [monitors, pointer] = await Promise.all([
    availableMonitors(),
    cursorPosition(),
  ]);
  const monitor = selectMonitorForPoint(monitors, pointer);

  if (!monitor) return;

  const scale = monitor.scaleFactor;
  const x = monitor.position.x +
    Math.round((monitor.size.width - OVERLAY_WIDTH * scale) / 2);
  const y = monitor.position.y + Math.round(TOP_MARGIN * scale);
  await getCurrentWindow().setPosition(new PhysicalPosition(x, y));
}

export function MeetingDetectionOverlay() {
  const [detection, setDetection] = useState<ConferenceCallDetected | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isDismissing, setIsDismissing] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const hide = useCallback(async () => {
    setDetection(null);
    setActionError(null);
    setIsStarting(false);
    setIsDismissing(false);
    await getCurrentWindow().hide();
  }, []);

  useEffect(() => {
    const previousBodyBackground = document.body.style.background;
    const previousHtmlBackground = document.documentElement.style.background;
    document.body.style.background = 'transparent';
    document.documentElement.style.background = 'transparent';

    return () => {
      document.body.style.background = previousBodyBackground;
      document.documentElement.style.background = previousHtmlBackground;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenDetected = await listen<ConferenceCallDetected>(
        'conference-call-detected',
        async ({ payload }) => {
          try {
            const mainWindow = await TauriWindow.getByLabel('main');
            if (cancelled || await mainWindow?.isFocused()) {
              return;
            }

            setActionError(null);
            setDetection(payload);
            try {
              await positionOnActiveMonitor();
            } catch (error) {
              console.debug('[MeetingDetectionOverlay] Positioning failed:', error);
            }
            if (!cancelled) {
              await getCurrentWindow().show();
            }
          } catch (error) {
            console.error('[MeetingDetectionOverlay] Failed to show prompt:', error);
          }
        },
      );
      if (cancelled) {
        unlistenDetected();
        return;
      }
      unlisteners.push(unlistenDetected);

      const unlistenEnded = await listen<ConferenceCallEnded>(
        'conference-call-ended',
        ({ payload }) => {
          setDetection((current) => {
            if (current?.session_id === payload.session_id) {
              void getCurrentWindow().hide();
              return null;
            }
            return current;
          });
        },
      );
      if (cancelled) {
        unlistenEnded();
        unlisteners.forEach((unlisten) => unlisten());
        return;
      }
      unlisteners.push(unlistenEnded);

      const unlistenHide = await listen('meeting-detection-overlay-hide', () => {
        void hide().catch((error) => {
          console.debug('[MeetingDetectionOverlay] Failed to hide prompt:', error);
        });
      });
      if (cancelled) {
        unlistenHide();
        unlisteners.forEach((unlisten) => unlisten());
        return;
      }
      unlisteners.push(unlistenHide);
    };

    void setup().catch((error) => {
      console.error('[MeetingDetectionOverlay] Failed to initialize:', error);
      setActionError('The meeting prompt could not initialize.');
    });
    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [hide]);

  const startRecording = useCallback(async () => {
    if (!detection || isStarting || isDismissing) return;
    setActionError(null);
    setIsStarting(true);

    const requestId = crypto.randomUUID();
    let waiter: Awaited<ReturnType<typeof createRecordingResultWaiter>> | null = null;
    try {
      waiter = await createRecordingResultWaiter(requestId);
      await emitTo('main', MEETING_DETECTION_START_REQUEST, {
        request_id: requestId,
        session_id: detection.session_id,
      });
      const result = await waiter.promise;
      const mainWindow = await TauriWindow.getByLabel('main');

      if (recordingStartSucceeded(result)) {
        await hide();
        await mainWindow?.show();
        await mainWindow?.setFocus();
        return;
      }

      if (result.status === 'blocked') {
        await hide();
        await mainWindow?.show();
        await mainWindow?.setFocus();
        return;
      }

      setActionError(result.message ?? 'Recording could not be started.');
    } catch (error) {
      console.error('[MeetingDetectionOverlay] Failed to request recording:', error);
      setActionError(
        error instanceof Error ? error.message : 'Recording could not be started.',
      );
    } finally {
      waiter?.cleanup();
      setIsStarting(false);
    }
  }, [detection, hide, isDismissing, isStarting]);

  const dismiss = useCallback(async () => {
    if (!detection || isStarting || isDismissing) return;
    const sessionId = detection.session_id;
    setActionError(null);
    setIsDismissing(true);
    try {
      const dismissed = await invoke<boolean>(
        'dismiss_meeting_detection_session',
        { sessionId },
      );
      if (!dismissed) {
        throw new Error('This meeting prompt is no longer active.');
      }
      await hide();
    } catch (error) {
      console.error('[MeetingDetectionOverlay] Failed to dismiss session:', error);
      setActionError(
        error instanceof Error ? error.message : 'The meeting prompt could not be dismissed.',
      );
      setIsDismissing(false);
    }
  }, [detection, hide, isDismissing, isStarting]);

  if (!detection) return null;

  return (
    <div className="h-screen w-screen p-2">
      <section
        aria-label={`${detection.app_name} meeting detected`}
        aria-live="assertive"
        className="flex h-full w-full items-center gap-3 rounded-xl border border-gray-200 bg-white px-4 shadow-2xl"
      >
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-blue-50 text-blue-700">
          <Mic className="h-4 w-4" aria-hidden="true" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-gray-950">
            {detection.confidence === 'High'
              ? 'Meeting detected'
              : 'Possible meeting detected'}
          </p>
          <p className={`truncate text-xs ${actionError ? 'text-red-600' : 'text-gray-600'}`}>
            {actionError ??
              `${detection.app_name} appears to be using your microphone.`}
          </p>
        </div>
        <button
          type="button"
          onClick={dismiss}
          disabled={isStarting || isDismissing}
          className="rounded-md px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          {isDismissing ? 'Dismissing…' : 'Dismiss'}
        </button>
        <button
          type="button"
          onClick={startRecording}
          disabled={isStarting || isDismissing}
          className="whitespace-nowrap rounded-md bg-gray-950 px-3 py-2 text-xs font-semibold text-white hover:bg-gray-800 focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          {isStarting ? 'Starting…' : 'Start Recording'}
        </button>
        <button
          type="button"
          aria-label="Dismiss meeting prompt"
          onClick={dismiss}
          disabled={isStarting || isDismissing}
          className="rounded-md p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </section>
    </div>
  );
}
