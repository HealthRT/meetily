use cidre::{core_audio as ca, ns};

use super::ObservationProvider;
use crate::audio::meeting_detection::types::ProcessObservation;

/// Metadata-only Core Audio observer. It never creates an aggregate device, tap, or stream.
#[derive(Debug, Default)]
pub struct MacOSObservationProvider;

impl ObservationProvider for MacOSObservationProvider {
    fn observations(&self) -> Vec<ProcessObservation> {
        // Core Audio process objects and their input/output activity properties are available
        // on macOS 14.2+. On older systems `processes` fails and detection remains inert.
        let Ok(processes) = ca::System::processes() else {
            return Vec::new();
        };

        processes
            .into_iter()
            .filter_map(|process| {
                let input_active = process.is_running_input().unwrap_or(false);
                let output_active = process.is_running_output().unwrap_or(false);
                if !input_active && !output_active {
                    return None;
                }

                let pid = process.pid().ok()?;
                let bundle_id = process.bundle_id().ok().map(|value| value.to_string());
                let running_app = ns::RunningApp::with_pid(pid);
                let name = running_app
                    .as_ref()
                    .and_then(|app| app.localized_name())
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| format!("Process {pid}"));

                Some(ProcessObservation {
                    pid,
                    bundle_id,
                    name,
                    input_active,
                    output_active,
                })
            })
            .collect()
    }
}
