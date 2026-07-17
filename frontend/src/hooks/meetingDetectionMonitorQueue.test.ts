import { describe, expect, it, vi } from 'vitest';
import {
  createMeetingDetectionMonitorQueue,
  MeetingDetectionMonitorCommand,
} from './meetingDetectionMonitorQueue';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

describe('createMeetingDetectionMonitorQueue', () => {
  it('serializes rapid stop and restart transitions', async () => {
    const firstStart = deferred();
    const commands: MeetingDetectionMonitorCommand[] = [];
    const invokeCommand = vi.fn((command: MeetingDetectionMonitorCommand) => {
      commands.push(command);
      return commands.length === 1 ? firstStart.promise : Promise.resolve();
    });
    const monitor = createMeetingDetectionMonitorQueue(invokeCommand);

    const start = monitor.setEnabled(true);
    const stop = monitor.setEnabled(false);
    const restart = monitor.setEnabled(true);
    await Promise.resolve();

    expect(commands).toEqual(['start_system_audio_monitoring']);

    firstStart.resolve();
    await Promise.all([start, stop, restart]);
    expect(commands).toEqual([
      'start_system_audio_monitoring',
      'stop_system_audio_monitoring',
      'start_system_audio_monitoring',
    ]);
  });

  it('continues processing after an IPC failure', async () => {
    const invokeCommand = vi
      .fn<(command: MeetingDetectionMonitorCommand) => Promise<void>>()
      .mockRejectedValueOnce(new Error('start failed'))
      .mockResolvedValue(undefined);
    const monitor = createMeetingDetectionMonitorQueue(invokeCommand);

    await expect(monitor.setEnabled(true)).rejects.toThrow('start failed');
    await expect(monitor.setEnabled(false)).resolves.toBeUndefined();
    expect(invokeCommand).toHaveBeenLastCalledWith('stop_system_audio_monitoring');
  });
});
