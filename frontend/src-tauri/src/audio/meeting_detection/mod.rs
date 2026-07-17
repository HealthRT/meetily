pub mod classifier;
pub mod platform;
pub mod session;
pub mod types;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use platform::{ObservationProvider, SystemObservationProvider};
use session::{Clock, SharedSessionTracker, SystemClock};

pub use types::{
    new_meeting_detection_callback, DetectionConfidence, MeetingApp, MeetingDetectionCallback,
    MeetingDetectionEvent, MeetingEndedEvent, MeetingStartedEvent, ProcessObservation,
};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
type SuppressionProbe = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

/// Plugin-shaped detector whose platform adapter supplies process metadata only.
pub struct MeetingMetadataDetector<
    P: ObservationProvider = SystemObservationProvider,
    C: Clock = SystemClock,
> {
    provider: Arc<P>,
    sessions: Arc<SharedSessionTracker<C>>,
    recording_active: Arc<AtomicBool>,
    suppression_probe: SuppressionProbe,
    task: Option<tokio::task::JoinHandle<()>>,
    stop_sender: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Default for MeetingMetadataDetector<SystemObservationProvider, SystemClock> {
    fn default() -> Self {
        Self::with_components(SystemObservationProvider::default(), SystemClock::default())
    }
}

impl MeetingMetadataDetector<SystemObservationProvider, SystemClock> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<P: ObservationProvider, C: Clock> MeetingMetadataDetector<P, C> {
    pub fn with_components(provider: P, clock: C) -> Self {
        Self {
            provider: Arc::new(provider),
            sessions: Arc::new(SharedSessionTracker::new(clock)),
            recording_active: Arc::new(AtomicBool::new(false)),
            suppression_probe: Arc::new(|| false),
            task: None,
            stop_sender: None,
        }
    }

    pub fn start(&mut self, callback: MeetingDetectionCallback) {
        if self.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let provider = Arc::clone(&self.provider);
        let sessions = Arc::clone(&self.sessions);
        let recording_active = Arc::clone(&self.recording_active);
        let suppression_probe = Arc::clone(&self.suppression_probe);
        let (stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();

        self.task = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(POLL_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = &mut stop_receiver => break,
                    _ = interval.tick() => {
                        let events = sessions.update(
                            provider.observations(),
                            recording_active.load(Ordering::Acquire) || suppression_probe(),
                        );
                        for event in events {
                            callback(event);
                        }
                    }
                }
            }
        }));
        self.stop_sender = Some(stop_sender);
    }

    pub fn stop(&mut self) {
        if let Some(stop_sender) = self.stop_sender.take() {
            let _ = stop_sender.send(());
        }
        if let Some(task) = self.task.take() {
            // Abort also makes stop deterministic if a provider call is unexpectedly slow.
            task.abort();
        }
    }

    pub fn set_recording_active(&self, active: bool) {
        self.recording_active.store(active, Ordering::Release);
    }

    pub fn set_suppression_probe<F>(&mut self, probe: F)
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.suppression_probe = Arc::new(probe);
    }

    pub fn recording_suppression(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.recording_active)
    }

    pub fn dismiss_session(&self, session_id: &str) -> bool {
        self.sessions.dismiss(session_id)
    }

    /// Runs one synchronous cycle; useful for embedding and deterministic tests.
    pub fn tick(&self) -> Vec<MeetingDetectionEvent> {
        self.sessions.update(
            self.provider.observations(),
            self.recording_active.load(Ordering::Acquire) || (self.suppression_probe)(),
        )
    }
}

impl<P: ObservationProvider, C: Clock> Drop for MeetingMetadataDetector<P, C> {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    };

    use super::*;

    #[derive(Default)]
    struct ManualClock(AtomicU64);

    impl ManualClock {
        fn set(&self, seconds: u64) {
            self.0.store(seconds, Ordering::Relaxed);
        }
    }

    impl Clock for Arc<ManualClock> {
        fn now(&self) -> Duration {
            Duration::from_secs(self.0.load(Ordering::Relaxed))
        }
    }

    #[derive(Default)]
    struct InjectedObservations(Mutex<Vec<ProcessObservation>>);

    impl InjectedObservations {
        fn set(&self, observations: Vec<ProcessObservation>) {
            *self.0.lock().unwrap() = observations;
        }
    }

    impl ObservationProvider for Arc<InjectedObservations> {
        fn observations(&self) -> Vec<ProcessObservation> {
            self.0.lock().unwrap().clone()
        }
    }

    #[test]
    fn accepts_injected_clock_and_observations() {
        let clock = Arc::new(ManualClock::default());
        let observations = Arc::new(InjectedObservations::default());
        observations.set(vec![ProcessObservation {
            pid: 1,
            bundle_id: Some("us.zoom.xos".to_owned()),
            name: "zoom.us".to_owned(),
            input_active: true,
            output_active: true,
        }]);
        let detector =
            MeetingMetadataDetector::with_components(observations.clone(), clock.clone());

        assert!(detector.tick().is_empty());
        clock.set(3);
        assert!(matches!(
            detector.tick().as_slice(),
            [MeetingDetectionEvent::MeetingStarted(_)]
        ));
    }

    #[test]
    fn external_suppression_probe_prevents_session_promotion() {
        let clock = Arc::new(ManualClock::default());
        let observations = Arc::new(InjectedObservations::default());
        observations.set(vec![ProcessObservation {
            pid: 1,
            bundle_id: Some("us.zoom.xos".to_owned()),
            name: "zoom.us".to_owned(),
            input_active: true,
            output_active: true,
        }]);
        let suppressed = Arc::new(AtomicBool::new(true));
        let mut detector =
            MeetingMetadataDetector::with_components(observations.clone(), clock.clone());
        detector.set_suppression_probe({
            let suppressed = Arc::clone(&suppressed);
            move || suppressed.load(Ordering::Relaxed)
        });

        assert!(detector.tick().is_empty());
        clock.set(3);
        assert!(detector.tick().is_empty());

        suppressed.store(false, Ordering::Relaxed);
        clock.set(4);
        assert!(detector.tick().is_empty());
        clock.set(7);
        assert!(matches!(
            detector.tick().as_slice(),
            [MeetingDetectionEvent::MeetingStarted(_)]
        ));
    }

    #[tokio::test]
    async fn stop_terminates_without_native_threads() {
        let mut detector = MeetingMetadataDetector::with_components(
            Arc::new(InjectedObservations::default()),
            Arc::new(ManualClock::default()),
        );
        detector.start(new_meeting_detection_callback(|_| {}));
        assert!(detector
            .task
            .as_ref()
            .is_some_and(|task| !task.is_finished()));
        detector.stop();
        assert!(detector.task.is_none());
        assert!(detector.stop_sender.is_none());
    }
}
