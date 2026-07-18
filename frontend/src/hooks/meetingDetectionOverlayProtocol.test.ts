import { describe, expect, it } from 'vitest';
import {
  recordingStartSucceeded,
  selectMonitorForPoint,
} from './meetingDetectionOverlayProtocol';

const monitors = [
  {
    name: 'left',
    position: { x: -1920, y: 0 },
    size: { width: 1920, height: 1080 },
  },
  {
    name: 'main',
    position: { x: 0, y: 0 },
    size: { width: 2560, height: 1440 },
  },
];

describe('selectMonitorForPoint', () => {
  it('selects a secondary monitor with negative coordinates', () => {
    expect(selectMonitorForPoint(monitors, { x: -400, y: 300 })?.name)
      .toBe('left');
  });

  it('uses the first monitor when the point is outside every display', () => {
    expect(selectMonitorForPoint(monitors, { x: 9000, y: 9000 })?.name)
      .toBe('left');
  });
});

describe('recordingStartSucceeded', () => {
  it.each(['started', 'already-recording'] as const)(
    'accepts %s as a successful acknowledgement',
    (status) => {
      expect(recordingStartSucceeded({ request_id: '1', status })).toBe(true);
    },
  );

  it.each(['blocked', 'failed'] as const)(
    'rejects %s as a successful acknowledgement',
    (status) => {
      expect(recordingStartSucceeded({ request_id: '1', status })).toBe(false);
    },
  );
});
