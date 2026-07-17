'use client';

import { useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  isPermissionGranted,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { useConfig } from '@/contexts/ConfigContext';

interface ConferenceCallDetected {
  version: number;
  session_id: string;
  app_id: string;
  app_name: string;
  confidence: 'High' | 'Low';
  detected_at: string;
}

interface ConferenceCallEnded {
  session_id: string;
}

const toastIdForSession = (sessionId: string) =>
  `meeting-detection-${sessionId}`;

export function useMeetingDetection(onboardingCompleted: boolean) {
  const router = useRouter();
  const {
    notificationSettings,
    loadPreferences,
  } = useConfig();
  const seenSessionsRef = useRef(new Set<string>());
  const activeSessionsRef = useRef(new Set<string>());
  const detectionEnabledRef = useRef(false);
  const monitorCommandQueueRef = useRef<Promise<void>>(Promise.resolve());

  const setMonitoring = useCallback((enabled: boolean) => {
    const command = enabled
      ? 'start_system_audio_monitoring'
      : 'stop_system_audio_monitoring';
    const operation = monitorCommandQueueRef.current.then(
      () => invoke<void>(command),
      () => invoke<void>(command),
    );
    monitorCommandQueueRef.current = operation.catch(() => undefined);
    return operation;
  }, []);

  const dismissPrompt = useCallback((sessionId: string) => {
    toast.dismiss(toastIdForSession(sessionId));
    activeSessionsRef.current.delete(sessionId);
  }, []);

  const dismissAllPrompts = useCallback(() => {
    activeSessionsRef.current.forEach((sessionId) => {
      toast.dismiss(toastIdForSession(sessionId));
    });
    activeSessionsRef.current.clear();
  }, []);

  const startRecording = useCallback((sessionId: string) => {
    dismissPrompt(sessionId);

    if (window.location.pathname === '/') {
      window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
      return;
    }

    sessionStorage.setItem('autoStartRecording', 'true');
    router.push('/');
  }, [dismissPrompt, router]);

  const dismissSession = useCallback((sessionId: string) => {
    dismissPrompt(sessionId);
    invoke('dismiss_meeting_detection_session', { sessionId }).catch((error) => {
      console.error('[MeetingDetection] Failed to dismiss session:', error);
    });
  }, [dismissPrompt]);

  const showHiddenWindowNotification = useCallback(async (
    detection: ConferenceCallDetected,
  ) => {
    try {
      if (!detectionEnabledRef.current) return;
      if (await getCurrentWindow().isFocused()) return;
      if (!detectionEnabledRef.current) return;
      if (!(await isPermissionGranted())) return;
      if (!detectionEnabledRef.current) return;

      sendNotification({
        title: detection.confidence === 'High'
          ? 'Meeting detected'
          : 'Possible meeting detected',
        body: detection.confidence === 'High'
          ? `Open Meetily to choose whether to record the call in ${detection.app_name}.`
          : `${detection.app_name} is using the microphone. Open Meetily to choose whether to record.`,
      });
    } catch (error) {
      // A denied permission or unavailable notification API must not disrupt detection.
      console.debug('[MeetingDetection] System notification unavailable:', error);
    }
  }, []);

  const handleDetected = useCallback((detection: ConferenceCallDetected) => {
    if (
      !detectionEnabledRef.current ||
      !detection.session_id ||
      seenSessionsRef.current.has(detection.session_id)
    ) {
      return;
    }

    seenSessionsRef.current.add(detection.session_id);
    activeSessionsRef.current.add(detection.session_id);

    const title = detection.confidence === 'High'
      ? 'Meeting detected'
      : 'Possible meeting detected';

    const description = detection.confidence === 'High'
      ? `${detection.app_name} appears to be in a conference call. Start recording?`
      : `${detection.app_name} is using the microphone. Start meeting notes?`;

    toast(title, {
      id: toastIdForSession(detection.session_id),
      description,
      duration: Infinity,
      dismissible: false,
      closeButton: false,
      action: {
        label: 'Start Recording',
        onClick: () => startRecording(detection.session_id),
      },
      cancel: {
        label: 'Dismiss',
        onClick: () => dismissSession(detection.session_id),
      },
    });

    void showHiddenWindowNotification(detection);
  }, [dismissSession, showHiddenWindowNotification, startRecording]);

  useEffect(() => {
    if (onboardingCompleted) {
      void loadPreferences();
    }
  }, [loadPreferences, onboardingCompleted]);

  useEffect(() => {
    const enabled =
      onboardingCompleted &&
      notificationSettings?.meeting_detection_enabled === true &&
      notificationSettings.manual_dnd_mode !== true;

    if (!enabled) {
      detectionEnabledRef.current = false;
      dismissAllPrompts();
      setMonitoring(false).catch((error) => {
        console.debug('[MeetingDetection] Monitor was not running:', error);
      });
      return;
    }

    detectionEnabledRef.current = true;
    let cancelled = false;
    let startRequested = false;
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      try {
        const supported = await invoke<boolean>('is_meeting_detection_supported');
        if (!supported || cancelled) return;

        const unlistenDetected = await listen<ConferenceCallDetected>(
          'conference-call-detected',
          (event) => handleDetected(event.payload),
        );
        if (cancelled) {
          unlistenDetected();
          return;
        }
        unlisteners.push(unlistenDetected);

        const unlistenEnded = await listen<ConferenceCallEnded>(
          'conference-call-ended',
          (event) => dismissPrompt(event.payload.session_id),
        );
        if (cancelled) {
          unlistenEnded();
          unlisteners.forEach((unlisten) => unlisten());
          return;
        }
        unlisteners.push(unlistenEnded);

        startRequested = true;
        await setMonitoring(true);
      } catch (error) {
        if (!cancelled) {
          console.error('[MeetingDetection] Failed to start monitoring:', error);
        }
      }
    };

    void setup();

    return () => {
      cancelled = true;
      detectionEnabledRef.current = false;
      unlisteners.forEach((unlisten) => unlisten());
      dismissAllPrompts();
      if (startRequested) {
        setMonitoring(false).catch((error) => {
          console.debug('[MeetingDetection] Failed to stop monitoring:', error);
        });
      }
    };
  }, [
    dismissAllPrompts,
    dismissPrompt,
    handleDetected,
    notificationSettings?.meeting_detection_enabled,
    notificationSettings?.manual_dnd_mode,
    onboardingCompleted,
    setMonitoring,
  ]);
}
