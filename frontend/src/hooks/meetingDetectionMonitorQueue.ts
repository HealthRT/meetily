export type MeetingDetectionMonitorCommand =
  | 'start_system_audio_monitoring'
  | 'stop_system_audio_monitoring';

export type MeetingDetectionMonitorInvoker = (
  command: MeetingDetectionMonitorCommand,
) => Promise<void>;

export interface MeetingDetectionMonitorQueue {
  setEnabled(enabled: boolean): Promise<void>;
}

export function createMeetingDetectionMonitorQueue(
  invokeCommand: MeetingDetectionMonitorInvoker,
): MeetingDetectionMonitorQueue {
  let queue = Promise.resolve();

  return {
    setEnabled(enabled: boolean) {
      const command = enabled
        ? 'start_system_audio_monitoring'
        : 'stop_system_audio_monitoring';
      const operation = queue.then(
        () => invokeCommand(command),
        () => invokeCommand(command),
      );

      // Keep subsequent transitions ordered even if one IPC call fails.
      queue = operation.catch(() => undefined);
      return operation;
    },
  };
}
