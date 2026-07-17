use std::sync::Arc;

use super::meeting_detection::{
    new_meeting_detection_callback, MeetingDetectionCallback, MeetingDetectionEvent,
    MeetingMetadataDetector,
};

/// Compatibility event API used by the existing Tauri system-audio commands.
#[derive(Debug, Clone)]
pub enum SystemAudioEvent {
    SystemAudioStarted(Vec<String>),
    SystemAudioStopped,
}

pub type SystemAudioCallback = Arc<dyn Fn(SystemAudioEvent) + Send + Sync + 'static>;

pub fn new_system_audio_callback<F>(callback: F) -> SystemAudioCallback
where
    F: Fn(SystemAudioEvent) + Send + Sync + 'static,
{
    Arc::new(callback)
}

/// Compatibility wrapper over the metadata-only meeting detector.
///
/// Unlike the former implementation, this does not observe the default output device,
/// install Core Audio listeners, create capture streams, or park a native thread.
#[derive(Default)]
pub struct SystemAudioDetector {
    inner: MeetingMetadataDetector,
}

impl SystemAudioDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, callback: SystemAudioCallback) {
        self.inner
            .start(new_meeting_detection_callback(move |event| match event {
                MeetingDetectionEvent::MeetingStarted(started) => {
                    callback(SystemAudioEvent::SystemAudioStarted(vec![started.app_name]));
                }
                MeetingDetectionEvent::MeetingEnded(_) => {
                    callback(SystemAudioEvent::SystemAudioStopped);
                }
            }));
    }

    pub fn start_meeting_detection(&mut self, callback: MeetingDetectionCallback) {
        self.inner.start(callback);
    }

    pub fn set_recording_active(&self, active: bool) {
        self.inner.set_recording_active(active);
    }

    pub fn set_suppression_probe<F>(&mut self, probe: F)
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.inner.set_suppression_probe(probe);
    }

    pub fn dismiss_session(&self, session_id: &str) -> bool {
        self.inner.dismiss_session(session_id)
    }

    pub fn stop(&mut self) {
        self.inner.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires macOS 14.2+ Core Audio process metadata and audio hardware"]
    async fn test_system_audio_detector_hardware() {
        let mut detector = SystemAudioDetector::new();
        detector.start(new_system_audio_callback(|event| {
            println!("System audio event: {event:?}");
        }));
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        detector.stop();
    }
}
