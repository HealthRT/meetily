use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

use super::{
    classifier::classify_process,
    types::{
        ClassifiedProcess, DetectionConfidence, MeetingApp, MeetingDetectionEvent,
        MeetingEndedEvent, MeetingStartedEvent, ProcessObservation,
    },
};

pub const START_DEBOUNCE: Duration = Duration::from_secs(3);
pub const END_GRACE: Duration = Duration::from_secs(5);
pub const DISMISSAL_COOLDOWN: Duration = Duration::from_secs(30 * 60);

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Duration;
}

pub struct SystemClock {
    epoch: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.epoch.elapsed()
    }
}

#[derive(Debug, Clone)]
struct Aggregate {
    app: MeetingApp,
    confidence: DetectionConfidence,
    process_ids: Vec<i32>,
    input_active: bool,
    output_active: bool,
}

#[derive(Debug)]
struct PendingSession {
    aggregate: Aggregate,
    first_seen: Duration,
}

#[derive(Debug)]
struct ActiveSession {
    id: String,
    aggregate: Aggregate,
    missing_since: Option<Duration>,
}

/// Pure session state machine. Time and process observations are injected by callers.
pub struct MeetingSessionTracker {
    pending: Option<PendingSession>,
    active: Option<ActiveSession>,
    cooldown_until: HashMap<MeetingApp, Duration>,
}

impl Default for MeetingSessionTracker {
    fn default() -> Self {
        Self {
            pending: None,
            active: None,
            cooldown_until: HashMap::new(),
        }
    }
}

impl MeetingSessionTracker {
    pub fn update(
        &mut self,
        now: Duration,
        observations: Vec<ProcessObservation>,
        recording_active: bool,
    ) -> Vec<MeetingDetectionEvent> {
        let mut aggregates = aggregate_observations(observations);
        let mut events = Vec::with_capacity(1);

        if recording_active {
            self.pending = None;
            if let Some(active) = self.active.take() {
                events.push(MeetingDetectionEvent::MeetingEnded(MeetingEndedEvent {
                    session_id: active.id,
                    app: active.aggregate.app,
                    app_name: active.aggregate.app.display_name().to_owned(),
                }));
            }
            return events;
        }

        if let Some(active) = self.active.as_mut() {
            if let Some(index) = aggregates
                .iter()
                .position(|item| item.app == active.aggregate.app)
            {
                let current = aggregates.swap_remove(index);
                active.aggregate = current;
                active.missing_since = None;
            } else {
                let missing_since = active.missing_since.get_or_insert(now);
                if now.saturating_sub(*missing_since) >= END_GRACE {
                    let ended = self.active.take().expect("active session exists");
                    events.push(MeetingDetectionEvent::MeetingEnded(MeetingEndedEvent {
                        session_id: ended.id,
                        app: ended.aggregate.app,
                        app_name: ended.aggregate.app.display_name().to_owned(),
                    }));
                }
            }

            return events;
        }

        self.cooldown_until.retain(|_, until| now < *until);
        let Some(aggregate) = aggregates
            .into_iter()
            .filter(|aggregate| !self.cooldown_until.contains_key(&aggregate.app))
            .max_by_key(aggregate_priority)
        else {
            self.pending = None;
            return events;
        };

        match self.pending.as_mut() {
            Some(pending) if pending.aggregate.app == aggregate.app => {
                pending.aggregate = aggregate;
                if now.saturating_sub(pending.first_seen) >= START_DEBOUNCE {
                    let pending = self.pending.take().expect("pending session exists");
                    let id = format!("meeting-detection-{}", uuid::Uuid::new_v4());
                    let started = MeetingStartedEvent {
                        session_id: id.clone(),
                        app: pending.aggregate.app,
                        app_name: pending.aggregate.app.display_name().to_owned(),
                        confidence: pending.aggregate.confidence,
                        process_ids: pending.aggregate.process_ids.clone(),
                        input_active: pending.aggregate.input_active,
                        output_active: pending.aggregate.output_active,
                    };
                    self.active = Some(ActiveSession {
                        id,
                        aggregate: pending.aggregate,
                        missing_since: None,
                    });
                    events.push(MeetingDetectionEvent::MeetingStarted(started));
                }
            }
            _ => {
                self.pending = Some(PendingSession {
                    aggregate,
                    first_seen: now,
                });
            }
        }

        events
    }

    /// Suppresses the active application family for the configured cooldown.
    /// A stale frontend action cannot dismiss a newer session.
    pub fn dismiss(&mut self, now: Duration, session_id: &str) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        if active.id != session_id {
            return false;
        }

        let app = active.aggregate.app;
        self.pending = None;
        self.active = None;
        self.cooldown_until
            .insert(app, now.saturating_add(DISMISSAL_COOLDOWN));
        true
    }

    pub fn has_active_session(&self) -> bool {
        self.active.is_some()
    }
}

fn aggregate_observations(observations: Vec<ProcessObservation>) -> Vec<Aggregate> {
    let mut grouped: HashMap<MeetingApp, Vec<ClassifiedProcess>> = HashMap::new();

    for observation in observations {
        if !observation.is_audio_active() {
            continue;
        }
        if let Some(classified) = classify_process(observation) {
            grouped.entry(classified.app).or_default().push(classified);
        }
    }

    grouped
        .into_iter()
        .map(|(app, processes)| {
            let mut seen = HashSet::new();
            let mut process_ids = processes
                .iter()
                .filter_map(|process| {
                    seen.insert(process.observation.pid)
                        .then_some(process.observation.pid)
                })
                .collect::<Vec<_>>();
            process_ids.sort_unstable();

            Aggregate {
                app,
                confidence: if app.is_browser() {
                    DetectionConfidence::Low
                } else {
                    DetectionConfidence::High
                },
                input_active: processes
                    .iter()
                    .any(|process| process.observation.input_active),
                output_active: processes
                    .iter()
                    .any(|process| process.observation.output_active),
                process_ids,
            }
        })
        // Output-only activity is ordinary playback, even for known meeting apps.
        .filter(|aggregate| aggregate.input_active)
        .collect()
}

fn aggregate_priority(aggregate: &Aggregate) -> (u8, u8, u8) {
    (
        u8::from(aggregate.confidence == DetectionConfidence::High),
        u8::from(aggregate.input_active),
        app_priority(aggregate.app),
    )
}

fn app_priority(app: MeetingApp) -> u8 {
    match app {
        MeetingApp::Zoom => 9,
        MeetingApp::MicrosoftTeams => 8,
        MeetingApp::FaceTime => 7,
        MeetingApp::Slack => 6,
        MeetingApp::Discord => 5,
        MeetingApp::GoogleChrome => 4,
        MeetingApp::MicrosoftEdge => 3,
        MeetingApp::Safari => 2,
        MeetingApp::Firefox => 1,
    }
}

pub struct SharedSessionTracker<C: Clock> {
    clock: C,
    tracker: Mutex<MeetingSessionTracker>,
}

impl<C: Clock> SharedSessionTracker<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            tracker: Mutex::new(MeetingSessionTracker::default()),
        }
    }

    pub fn update(
        &self,
        observations: Vec<ProcessObservation>,
        recording_active: bool,
    ) -> Vec<MeetingDetectionEvent> {
        self.tracker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .update(self.clock.now(), observations, recording_active)
    }

    pub fn dismiss(&self, session_id: &str) -> bool {
        self.tracker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .dismiss(self.clock.now(), session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        pid: i32,
        bundle_id: &str,
        input_active: bool,
        output_active: bool,
    ) -> ProcessObservation {
        ProcessObservation {
            pid,
            bundle_id: Some(bundle_id.to_owned()),
            name: "helper".to_owned(),
            input_active,
            output_active,
        }
    }

    fn update(
        tracker: &mut MeetingSessionTracker,
        seconds: u64,
        observations: Vec<ProcessObservation>,
        recording: bool,
    ) -> Vec<MeetingDetectionEvent> {
        tracker.update(Duration::from_secs(seconds), observations, recording)
    }

    #[test]
    fn debounces_start_for_three_seconds() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];

        assert!(update(&mut tracker, 0, zoom(), false).is_empty());
        assert!(update(&mut tracker, 2, zoom(), false).is_empty());
        let events = update(&mut tracker, 3, zoom(), false);

        assert!(matches!(
            events.as_slice(),
            [MeetingDetectionEvent::MeetingStarted(event)]
                if event.app == MeetingApp::Zoom
                    && event.confidence == DetectionConfidence::High
        ));
    }

    #[test]
    fn applies_five_second_end_grace_and_recovers_from_gaps() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        update(&mut tracker, 0, zoom(), false);
        update(&mut tracker, 3, zoom(), false);

        assert!(update(&mut tracker, 4, vec![], false).is_empty());
        assert!(update(&mut tracker, 8, zoom(), false).is_empty());
        assert!(update(&mut tracker, 9, vec![], false).is_empty());
        let events = update(&mut tracker, 14, vec![], false);
        assert!(matches!(
            events.as_slice(),
            [MeetingDetectionEvent::MeetingEnded(event)] if event.app == MeetingApp::Zoom
        ));
    }

    #[test]
    fn collapses_helpers_and_duplicate_pids_into_one_session() {
        let mut tracker = MeetingSessionTracker::default();
        let teams = || {
            vec![
                observation(8, "com.microsoft.teams2", true, false),
                observation(9, "com.microsoft.teams2.helper", false, true),
                observation(9, "com.microsoft.teams2.helper.renderer", false, true),
            ]
        };
        update(&mut tracker, 0, teams(), false);
        let events = update(&mut tracker, 3, teams(), false);

        match &events[0] {
            MeetingDetectionEvent::MeetingStarted(event) => {
                assert_eq!(event.process_ids, vec![8, 9]);
                assert!(event.input_active);
                assert!(event.output_active);
            }
            _ => panic!("expected meeting start"),
        }
    }

    #[test]
    fn browser_requires_input_and_has_low_confidence() {
        let mut tracker = MeetingSessionTracker::default();
        let playback = vec![observation(1, "com.google.Chrome", false, true)];
        assert!(update(&mut tracker, 0, playback, false).is_empty());
        assert!(update(&mut tracker, 4, vec![], false).is_empty());

        let call = || vec![observation(1, "com.google.Chrome", true, true)];
        update(&mut tracker, 5, call(), false);
        let events = update(&mut tracker, 8, call(), false);
        assert!(matches!(
            events.as_slice(),
            [MeetingDetectionEvent::MeetingStarted(event)]
                if event.confidence == DetectionConfidence::Low
        ));
    }

    #[test]
    fn recording_suppresses_pending_and_new_sessions() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        update(&mut tracker, 0, zoom(), false);
        assert!(update(&mut tracker, 3, zoom(), true).is_empty());
        assert!(!tracker.has_active_session());
        assert!(update(&mut tracker, 4, zoom(), false).is_empty());
        assert!(update(&mut tracker, 7, zoom(), false).len() == 1);
    }

    #[test]
    fn recording_ends_an_active_detection_session() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        update(&mut tracker, 0, zoom(), false);
        let started = update(&mut tracker, 3, zoom(), false);
        let session_id = match &started[0] {
            MeetingDetectionEvent::MeetingStarted(event) => event.session_id.clone(),
            _ => panic!("expected meeting start"),
        };

        let events = update(&mut tracker, 4, zoom(), true);
        assert!(matches!(
            events.as_slice(),
            [MeetingDetectionEvent::MeetingEnded(event)] if event.session_id == session_id
        ));
        assert!(!tracker.has_active_session());
        assert!(update(&mut tracker, 5, zoom(), true).is_empty());
    }

    #[test]
    fn dismissal_suppresses_for_thirty_minutes() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        update(&mut tracker, 0, zoom(), false);
        let events = update(&mut tracker, 3, zoom(), false);
        let session_id = match &events[0] {
            MeetingDetectionEvent::MeetingStarted(event) => event.session_id.clone(),
            _ => panic!("expected meeting start"),
        };
        assert!(tracker.dismiss(Duration::from_secs(4), &session_id));

        assert!(update(&mut tracker, 1_803, zoom(), false).is_empty());
        assert!(update(&mut tracker, 1_804, zoom(), false).is_empty());
        assert!(update(&mut tracker, 1_807, zoom(), false).len() == 1);
    }

    #[test]
    fn keeps_exactly_one_session_when_multiple_apps_are_active() {
        let mut tracker = MeetingSessionTracker::default();
        let observations = || {
            vec![
                observation(1, "com.google.Chrome", true, true),
                observation(2, "us.zoom.xos", true, true),
            ]
        };
        update(&mut tracker, 0, observations(), false);
        let events = update(&mut tracker, 3, observations(), false);
        assert!(matches!(
            events.as_slice(),
            [MeetingDetectionEvent::MeetingStarted(event)] if event.app == MeetingApp::Zoom
        ));
    }

    #[test]
    fn does_not_end_active_session_when_another_app_appears() {
        let mut tracker = MeetingSessionTracker::default();
        let chrome = || vec![observation(1, "com.google.Chrome", true, true)];
        update(&mut tracker, 0, chrome(), false);
        update(&mut tracker, 3, chrome(), false);

        let both = vec![
            observation(1, "com.google.Chrome", true, true),
            observation(2, "us.zoom.xos", true, true),
        ];
        assert!(update(&mut tracker, 4, both, false).is_empty());
        assert!(tracker.has_active_session());
    }

    #[test]
    fn stale_dismissal_cannot_suppress_a_newer_session() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        update(&mut tracker, 0, zoom(), false);
        update(&mut tracker, 3, zoom(), false);

        assert!(!tracker.dismiss(Duration::from_secs(4), "stale-session"));
        assert!(tracker.has_active_session());
    }

    #[test]
    fn session_ids_remain_unique_across_tracker_restarts() {
        let zoom = || vec![observation(1, "us.zoom.xos", true, true)];
        let start_session = || {
            let mut tracker = MeetingSessionTracker::default();
            update(&mut tracker, 0, zoom(), false);
            match &update(&mut tracker, 3, zoom(), false)[0] {
                MeetingDetectionEvent::MeetingStarted(event) => event.session_id.clone(),
                _ => panic!("expected meeting start"),
            }
        };

        assert_ne!(start_session(), start_session());
    }

    #[test]
    fn native_output_only_activity_does_not_start_a_session() {
        let mut tracker = MeetingSessionTracker::default();
        let zoom_playback = || vec![observation(1, "us.zoom.xos", false, true)];

        assert!(update(&mut tracker, 0, zoom_playback(), false).is_empty());
        assert!(update(&mut tracker, 4, zoom_playback(), false).is_empty());
        assert!(!tracker.has_active_session());
    }
}
