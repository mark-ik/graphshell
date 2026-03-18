#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryManagerTab {
    #[default]
    Timeline,
    Dissolved,
    /// Mixed multi-track timeline (all history tracks, filtered).
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryCaptureStatus {
    Full,
    DegradedCaptureOnly,
}

impl HistoryCaptureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::DegradedCaptureOnly => "degraded-capture-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryTraversalFailureReason {
    MissingOldUrl,
    MissingNewUrl,
    SameUrl,
    NonHistoryTransition,
    MissingEndpoint,
    SelfLoop,
    GraphRejected,
    PersistenceUnavailable,
    ExportWriteFailed,
    ExportReadFailed,
    HomeDirectoryUnavailable,
    PreviewIsolationViolation,
    ReplayFailed,
    ReturnToPresentFailed,
}

impl HistoryTraversalFailureReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingOldUrl => "missing_old_url",
            Self::MissingNewUrl => "missing_new_url",
            Self::SameUrl => "same_url",
            Self::NonHistoryTransition => "non_history_transition",
            Self::MissingEndpoint => "missing_endpoint",
            Self::SelfLoop => "self_loop",
            Self::GraphRejected => "graph_rejected",
            Self::PersistenceUnavailable => "persistence_unavailable",
            Self::ExportWriteFailed => "export_write_failed",
            Self::ExportReadFailed => "export_read_failed",
            Self::HomeDirectoryUnavailable => "home_directory_unavailable",
            Self::PreviewIsolationViolation => "preview_isolation_violation",
            Self::ReplayFailed => "replay_failed",
            Self::ReturnToPresentFailed => "return_to_present_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryHealthSummary {
    pub capture_status: HistoryCaptureStatus,
    pub recent_traversal_append_failures: u64,
    pub recent_failure_reason_bucket: Option<String>,
    pub last_error: Option<String>,
    pub traversal_archive_count: usize,
    pub dissolved_archive_count: usize,
    pub preview_mode_active: bool,
    pub last_preview_isolation_violation: bool,
    pub replay_in_progress: bool,
    pub replay_cursor: Option<usize>,
    pub replay_total_steps: Option<usize>,
    pub last_return_to_present_result: Option<String>,
    pub last_event_unix_ms: Option<u64>,
}
