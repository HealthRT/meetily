use super::types::{ClassifiedProcess, DetectionConfidence, MeetingApp, ProcessObservation};

/// Classifies by bundle identity first, then by conservative process-name fallbacks.
pub fn classify_process(observation: ProcessObservation) -> Option<ClassifiedProcess> {
    let app = observation
        .bundle_id
        .as_deref()
        .and_then(classify_bundle_id)
        .or_else(|| classify_process_name(&observation.name))?;

    Some(ClassifiedProcess {
        confidence: if app.is_browser() {
            DetectionConfidence::Low
        } else {
            DetectionConfidence::High
        },
        app,
        observation,
    })
}

fn classify_bundle_id(bundle_id: &str) -> Option<MeetingApp> {
    let id = bundle_id.trim().to_ascii_lowercase();

    if bundle_family(&id, "us.zoom.xos") {
        Some(MeetingApp::Zoom)
    } else if bundle_family(&id, "com.microsoft.teams")
        || bundle_family(&id, "com.microsoft.teams2")
    {
        Some(MeetingApp::MicrosoftTeams)
    } else if bundle_family(&id, "com.apple.facetime") {
        Some(MeetingApp::FaceTime)
    } else if bundle_family(&id, "com.tinyspeck.slackmacgap") {
        Some(MeetingApp::Slack)
    } else if bundle_family(&id, "com.hnc.discord") {
        Some(MeetingApp::Discord)
    } else if bundle_family(&id, "com.google.chrome") {
        Some(MeetingApp::GoogleChrome)
    } else if bundle_family(&id, "com.microsoft.edgemac") {
        Some(MeetingApp::MicrosoftEdge)
    } else if bundle_family(&id, "com.apple.safari") {
        Some(MeetingApp::Safari)
    } else if bundle_family(&id, "org.mozilla.firefox") {
        Some(MeetingApp::Firefox)
    } else {
        None
    }
}

fn bundle_family(value: &str, root: &str) -> bool {
    value == root
        || value
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('-'))
}

fn classify_process_name(name: &str) -> Option<MeetingApp> {
    let normalized = name.trim().to_ascii_lowercase().replace(['-', '_'], " ");
    let base = normalized
        .strip_suffix(" helper (renderer)")
        .or_else(|| normalized.strip_suffix(" helper (gpu)"))
        .or_else(|| normalized.strip_suffix(" helper (plugin)"))
        .or_else(|| normalized.strip_suffix(" helper"))
        .unwrap_or(&normalized)
        .trim();

    match base {
        "zoom" | "zoom.us" | "cpt host" => Some(MeetingApp::Zoom),
        "microsoft teams" | "ms teams" | "teams" => Some(MeetingApp::MicrosoftTeams),
        "facetime" => Some(MeetingApp::FaceTime),
        "slack" => Some(MeetingApp::Slack),
        "discord" => Some(MeetingApp::Discord),
        "google chrome" | "chrome" => Some(MeetingApp::GoogleChrome),
        "microsoft edge" | "edge" => Some(MeetingApp::MicrosoftEdge),
        "safari" | "safari web content" => Some(MeetingApp::Safari),
        "firefox" | "firefoxcp web content" => Some(MeetingApp::Firefox),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(bundle_id: Option<&str>, name: &str) -> ProcessObservation {
        ProcessObservation {
            pid: 42,
            bundle_id: bundle_id.map(str::to_owned),
            name: name.to_owned(),
            input_active: true,
            output_active: false,
        }
    }

    #[test]
    fn classifies_stable_native_bundle_families_as_high_confidence() {
        let cases = [
            ("us.zoom.xos", MeetingApp::Zoom),
            ("us.zoom.xos.helper", MeetingApp::Zoom),
            ("com.microsoft.teams2", MeetingApp::MicrosoftTeams),
            (
                "com.microsoft.teams2.helper.renderer",
                MeetingApp::MicrosoftTeams,
            ),
            ("com.apple.FaceTime", MeetingApp::FaceTime),
            ("com.tinyspeck.slackmacgap", MeetingApp::Slack),
            ("com.hnc.Discord", MeetingApp::Discord),
        ];

        for (bundle_id, expected) in cases {
            let classified = classify_process(observation(Some(bundle_id), "irrelevant")).unwrap();
            assert_eq!(classified.app, expected);
            assert_eq!(classified.confidence, DetectionConfidence::High);
        }
    }

    #[test]
    fn classifies_browser_families_as_low_confidence() {
        let cases = [
            ("com.google.Chrome.helper", MeetingApp::GoogleChrome),
            ("com.microsoft.edgemac", MeetingApp::MicrosoftEdge),
            ("com.apple.Safari", MeetingApp::Safari),
            ("org.mozilla.firefox", MeetingApp::Firefox),
        ];

        for (bundle_id, expected) in cases {
            let classified = classify_process(observation(Some(bundle_id), "irrelevant")).unwrap();
            assert_eq!(classified.app, expected);
            assert_eq!(classified.confidence, DetectionConfidence::Low);
        }
    }

    #[test]
    fn uses_conservative_name_fallbacks_and_rejects_substrings() {
        assert_eq!(
            classify_process(observation(None, "Microsoft Teams Helper"))
                .unwrap()
                .app,
            MeetingApp::MicrosoftTeams
        );
        assert_eq!(
            classify_process(observation(None, "Google Chrome Helper (Renderer)"))
                .unwrap()
                .app,
            MeetingApp::GoogleChrome
        );
        assert!(classify_process(observation(None, "TeamViewer")).is_none());
        assert!(classify_process(observation(None, "Discordant Audio")).is_none());
        assert!(
            classify_process(observation(Some("com.example.zoom-not-really"), "Unknown")).is_none()
        );
    }

    #[test]
    fn stable_bundle_identity_wins_over_display_name() {
        let classified = classify_process(observation(Some("com.apple.Safari"), "Zoom")).unwrap();
        assert_eq!(classified.app, MeetingApp::Safari);
    }
}
