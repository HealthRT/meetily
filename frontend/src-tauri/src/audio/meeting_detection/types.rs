use serde::{Deserialize, Serialize};

/// A supported application family. Helper processes are normalized to this identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingApp {
    Zoom,
    MicrosoftTeams,
    FaceTime,
    Slack,
    Discord,
    GoogleChrome,
    MicrosoftEdge,
    Safari,
    Firefox,
}

impl MeetingApp {
    pub fn id(self) -> &'static str {
        match self {
            Self::Zoom => "zoom",
            Self::MicrosoftTeams => "microsoft_teams",
            Self::FaceTime => "facetime",
            Self::Slack => "slack",
            Self::Discord => "discord",
            Self::GoogleChrome => "google_chrome",
            Self::MicrosoftEdge => "microsoft_edge",
            Self::Safari => "safari",
            Self::Firefox => "firefox",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Zoom => "Zoom",
            Self::MicrosoftTeams => "Microsoft Teams",
            Self::FaceTime => "FaceTime",
            Self::Slack => "Slack",
            Self::Discord => "Discord",
            Self::GoogleChrome => "Google Chrome",
            Self::MicrosoftEdge => "Microsoft Edge",
            Self::Safari => "Safari",
            Self::Firefox => "Firefox",
        }
    }

    pub fn is_browser(self) -> bool {
        matches!(
            self,
            Self::GoogleChrome | Self::MicrosoftEdge | Self::Safari | Self::Firefox
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessObservation {
    pub pid: i32,
    pub bundle_id: Option<String>,
    pub name: String,
    pub input_active: bool,
    pub output_active: bool,
}

impl ProcessObservation {
    pub fn is_audio_active(&self) -> bool {
        self.input_active || self.output_active
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedProcess {
    pub app: MeetingApp,
    pub confidence: DetectionConfidence,
    pub observation: ProcessObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingStartedEvent {
    pub session_id: String,
    pub app: MeetingApp,
    pub app_name: String,
    pub confidence: DetectionConfidence,
    pub process_ids: Vec<i32>,
    pub input_active: bool,
    pub output_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingEndedEvent {
    pub session_id: String,
    pub app: MeetingApp,
    pub app_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum MeetingDetectionEvent {
    MeetingStarted(MeetingStartedEvent),
    MeetingEnded(MeetingEndedEvent),
}

pub type MeetingDetectionCallback =
    std::sync::Arc<dyn Fn(MeetingDetectionEvent) + Send + Sync + 'static>;

pub fn new_meeting_detection_callback<F>(callback: F) -> MeetingDetectionCallback
where
    F: Fn(MeetingDetectionEvent) + Send + Sync + 'static,
{
    std::sync::Arc::new(callback)
}
