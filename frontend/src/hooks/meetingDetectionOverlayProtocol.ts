export const MEETING_DETECTION_START_REQUEST =
  'meeting-detection-start-recording';
export const MEETING_DETECTION_START_RESULT =
  'meeting-detection-recording-result';

export interface MeetingDetectionStartRequest {
  request_id: string;
  session_id: string;
}

export type MeetingDetectionStartStatus =
  | 'started'
  | 'already-recording'
  | 'blocked'
  | 'failed';

export interface MeetingDetectionStartResult {
  request_id: string;
  status: MeetingDetectionStartStatus;
  message?: string;
}

interface Point {
  x: number;
  y: number;
}

interface MonitorBounds {
  position: Point;
  size: {
    width: number;
    height: number;
  };
}

export function selectMonitorForPoint<T extends MonitorBounds>(
  monitors: T[],
  point: Point,
): T | undefined {
  return monitors.find(({ position, size }) =>
    point.x >= position.x &&
    point.x < position.x + size.width &&
    point.y >= position.y &&
    point.y < position.y + size.height
  ) ?? monitors[0];
}

export function recordingStartSucceeded(
  result: MeetingDetectionStartResult,
): boolean {
  return result.status === 'started' || result.status === 'already-recording';
}
