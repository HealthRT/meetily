use crate::audio::{
    check_system_audio_permissions, list_system_audio_devices, new_meeting_detection_callback,
    start_system_audio_capture, DetectionConfidence, MeetingDetectionEvent, SystemAudioDetector,
};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, State};

// Global state for system audio detector
type SystemAudioDetectorState = Arc<Mutex<Option<SystemAudioDetector>>>;

/// Start system audio capture (for capturing system output audio)
#[command]
pub async fn start_system_audio_capture_command() -> Result<String, String> {
    match start_system_audio_capture().await {
        Ok(_stream) => {
            // TODO: Store the stream in global state if needed for management
            Ok("System audio capture started successfully".to_string())
        }
        Err(e) => Err(format!("Failed to start system audio capture: {}", e)),
    }
}

/// List available system audio devices
#[command]
pub async fn list_system_audio_devices_command() -> Result<Vec<String>, String> {
    list_system_audio_devices().map_err(|e| format!("Failed to list system audio devices: {}", e))
}

/// Check if the app has permission to access system audio
#[command]
pub async fn check_system_audio_permissions_command() -> bool {
    check_system_audio_permissions()
}

/// Whether this build includes a native meeting-activity observation provider.
#[command]
pub fn is_meeting_detection_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        let Ok(output) = std::process::Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        return macos_version_supports_meeting_detection(
            String::from_utf8_lossy(&output.stdout).trim(),
        );
    }

    #[cfg(not(target_os = "macos"))]
    false
}

fn macos_version_supports_meeting_detection(version: &str) -> bool {
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse::<u32>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u32>().ok());

    matches!((major, minor), (Some(major), Some(minor)) if major > 14 || (major == 14 && minor >= 2))
}

/// Start monitoring system audio usage by other applications
#[command]
pub async fn start_system_audio_monitoring(
    app_handle: AppHandle,
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<(), String> {
    let mut detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    if detector_guard.is_some() {
        return Ok(());
    }

    let mut detector = SystemAudioDetector::new();
    detector.set_suppression_probe(crate::audio::recording_commands::is_recording_sync);

    // Create callback that emits privacy-minimal, typed meeting events.
    let callback = new_meeting_detection_callback(move |event| match event {
        MeetingDetectionEvent::MeetingStarted(started) => {
            if crate::audio::recording_commands::is_recording_sync() {
                tracing::debug!("Suppressing meeting prompt while Meetily is recording");
                return;
            }

            tracing::info!("Likely meeting detected: {}", started.app_name);
            let payload = ConferenceCallDetectedPayload {
                version: 1,
                session_id: started.session_id,
                app_id: started.app.id().to_owned(),
                app_name: started.app_name.clone(),
                confidence: match started.confidence {
                    DetectionConfidence::High => "High",
                    DetectionConfidence::Low => "Low",
                }
                .to_owned(),
                detected_at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = app_handle.emit("conference-call-detected", payload);
            let _ = app_handle.emit("system-audio-started", vec![started.app_name]);
        }
        MeetingDetectionEvent::MeetingEnded(ended) => {
            let payload = ConferenceCallEndedPayload {
                session_id: ended.session_id,
            };
            let _ = app_handle.emit("conference-call-ended", payload);
            let _ = app_handle.emit("system-audio-stopped", ());
            tracing::info!("Likely meeting ended: {}", ended.app_name);
        }
    });

    detector.start_meeting_detection(callback);
    *detector_guard = Some(detector);

    Ok(())
}

/// Stop monitoring system audio usage
#[command]
pub async fn stop_system_audio_monitoring(
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<(), String> {
    let mut detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    if let Some(mut detector) = detector_guard.take() {
        detector.stop();
    }
    Ok(())
}

/// Dismiss the active prompt and suppress that application family for 30 minutes.
#[command]
pub async fn dismiss_meeting_detection_session(
    session_id: String,
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<bool, String> {
    let detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    Ok(detector_guard
        .as_ref()
        .is_some_and(|detector| detector.dismiss_session(&session_id)))
}

/// Get the current status of system audio monitoring
#[command]
pub async fn get_system_audio_monitoring_status(
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<bool, String> {
    let detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    Ok(detector_guard.is_some())
}

/// Initialize the system audio detector state in Tauri app
pub fn init_system_audio_state() -> SystemAudioDetectorState {
    Arc::new(Mutex::new(None))
}

// Event payload types for frontend
#[derive(serde::Serialize, Clone)]
pub struct SystemAudioStartedPayload {
    pub apps: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct SystemAudioStoppedPayload;

#[derive(serde::Serialize, Clone)]
pub struct ConferenceCallDetectedPayload {
    pub version: u8,
    pub session_id: String,
    pub app_id: String,
    pub app_name: String,
    pub confidence: String,
    pub detected_at: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ConferenceCallEndedPayload {
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_system_audio_devices() {
        let devices = list_system_audio_devices_command().await;
        match devices {
            Ok(device_list) => {
                println!("System audio devices: {:?}", device_list);
                assert!(device_list.len() >= 0); // Should at least not crash
            }
            Err(e) => {
                println!("Error listing devices: {}", e);
                // This might fail on CI or systems without audio
            }
        }
    }

    #[tokio::test]
    async fn test_check_permissions() {
        let has_permission = check_system_audio_permissions_command().await;
        println!("Has system audio permissions: {}", has_permission);
        // This is mainly a smoke test to ensure it doesn't crash
    }

    #[test]
    fn meeting_detection_requires_macos_14_2_or_newer() {
        assert!(!macos_version_supports_meeting_detection("13.6.9"));
        assert!(!macos_version_supports_meeting_detection("14.1"));
        assert!(macos_version_supports_meeting_detection("14.2"));
        assert!(macos_version_supports_meeting_detection("15.0.1"));
        assert!(!macos_version_supports_meeting_detection("invalid"));
    }
}
