use super::*;

impl GraphBrowserApp {
    pub(crate) fn history_preview_blocks_intent(intent: &GraphIntent) -> bool {
        intent.as_graph_mutation().is_some()
            || intent.as_runtime_event().is_some()
            || matches!(
                intent.as_view_action(),
                Some(
                    ViewAction::SetNodePosition { .. }
                        | ViewAction::SetNodeFormDraft { .. }
                        | ViewAction::SetNodeThumbnail { .. }
                        | ViewAction::SetNodeFavicon { .. }
                )
            )
    }

    pub(crate) fn apply_history_preview_cursor(
        &mut self,
        cursor: usize,
        total_steps: usize,
    ) -> Result<(), String> {
        if !self.workspace.graph_runtime.history_preview_mode_active {
            return Err("history preview mode is not active".to_string());
        }

        if cursor == 0 {
            if let Some(snapshot) = self
                .workspace
                .graph_runtime
                .history_preview_live_graph_snapshot
                .as_ref()
            {
                self.workspace.graph_runtime.history_preview_graph = Some(snapshot.clone());
                return Ok(());
            }
            return Err("preview baseline graph is unavailable".to_string());
        }

        let Some(store) = self.services.persistence.as_ref() else {
            return Ok(());
        };

        let mut chronological = self.history_timeline_index_entries(total_steps.max(1));
        if chronological.is_empty() {
            return Err("timeline index is empty".to_string());
        }

        chronological.sort_by(|a, b| {
            a.timestamp_ms
                .cmp(&b.timestamp_ms)
                .then_with(|| a.log_position.cmp(&b.log_position))
        });

        let bounded_cursor = cursor.min(chronological.len());
        let target = chronological
            .get(bounded_cursor.saturating_sub(1))
            .ok_or_else(|| "replay cursor is out of bounds".to_string())?;

        let replay_graph = store
            .replay_to_timestamp(target.timestamp_ms)
            .ok_or_else(|| "replay_to_timestamp returned no graph".to_string())?;
        self.workspace.graph_runtime.history_preview_graph = Some(replay_graph);
        Ok(())
    }

    pub(crate) fn update_history_failure(
        &mut self,
        reason: HistoryTraversalFailureReason,
        detail: impl Into<String>,
    ) {
        let detail = detail.into();
        self.workspace
            .graph_runtime
            .history_recent_traversal_append_failures = self
            .workspace
            .graph_runtime
            .history_recent_traversal_append_failures
            .saturating_add(1);
        self.workspace
            .graph_runtime
            .history_recent_failure_reason_bucket = Some(reason);
        self.workspace.graph_runtime.history_last_error =
            Some(format!("{}: {}", reason.as_str(), detail));
        self.workspace.graph_runtime.history_last_event_unix_ms =
            Some(Self::unix_timestamp_ms_now());
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_HISTORY_TRAVERSAL_RECORD_FAILED,
            latency_us: 0,
        });
        log::warn!(
            "history traversal record failed: reason={} detail={}",
            reason.as_str(),
            detail
        );
    }

    pub(crate) fn apply_history_runtime_intent(&mut self, intent: GraphIntent) {
        match intent {
            GraphIntent::ClearHistoryTimeline => {
                if let Some(store) = &mut self.services.persistence {
                    store.clear_traversal_archive();
                    log::info!("Cleared traversal archive (Timeline)");
                } else {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::PersistenceUnavailable,
                        "clear timeline requested without persistence",
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::ClearHistoryDissolved => {
                if let Some(store) = &mut self.services.persistence {
                    store.clear_dissolved_archive();
                    log::info!("Cleared dissolved archive (Dissolved)");
                } else {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::PersistenceUnavailable,
                        "clear dissolved requested without persistence",
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::AutoCurateHistoryTimeline { keep_latest } => {
                if let Some(store) = &mut self.services.persistence {
                    let removed = store.auto_curate_traversal_archive(keep_latest);
                    log::info!(
                        "Auto-curated traversal archive: removed {} old entries (keep_latest={})",
                        removed,
                        keep_latest
                    );
                } else {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::PersistenceUnavailable,
                        "auto-curate timeline requested without persistence",
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::AutoCurateHistoryDissolved { keep_latest } => {
                if let Some(store) = &mut self.services.persistence {
                    let removed = store.auto_curate_dissolved_archive(keep_latest);
                    log::info!(
                        "Auto-curated dissolved archive: removed {} old entries (keep_latest={})",
                        removed,
                        keep_latest
                    );
                } else {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::PersistenceUnavailable,
                        "auto-curate dissolved requested without persistence",
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::ExportHistoryTimeline => {
                self.export_history_archive(false);
            }
            GraphIntent::ExportHistoryDissolved => {
                self.export_history_archive(true);
            }
            GraphIntent::EnterHistoryTimelinePreview => {
                self.workspace.graph_runtime.history_preview_mode_active = true;
                self.workspace
                    .graph_runtime
                    .history_last_preview_isolation_violation = false;
                self.workspace.graph_runtime.history_replay_in_progress = false;
                self.workspace.graph_runtime.history_replay_cursor = None;
                self.workspace.graph_runtime.history_replay_total_steps = None;
                self.workspace
                    .graph_runtime
                    .history_preview_live_graph_snapshot =
                    Some(self.workspace.domain.graph.clone());
                self.workspace.graph_runtime.history_preview_graph =
                    Some(self.workspace.domain.graph.clone());
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_TIMELINE_PREVIEW_ENTERED,
                    latency_us: 0,
                });
            }
            GraphIntent::ExitHistoryTimelinePreview => {
                if let Some(snapshot) = self
                    .workspace
                    .graph_runtime
                    .history_preview_live_graph_snapshot
                    .take()
                {
                    self.workspace.domain.graph = snapshot;
                    self.workspace
                        .graph_runtime
                        .history_last_return_to_present_result = Some("restored".to_string());
                }
                self.workspace.graph_runtime.history_preview_mode_active = false;
                self.workspace.graph_runtime.history_replay_in_progress = false;
                self.workspace.graph_runtime.history_preview_graph = None;
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_TIMELINE_PREVIEW_EXITED,
                    latency_us: 0,
                });
            }
            GraphIntent::HistoryTimelinePreviewIsolationViolation { detail } => {
                self.workspace
                    .graph_runtime
                    .history_last_preview_isolation_violation = true;
                self.workspace.graph_runtime.history_last_error = Some(format!(
                    "{}: {}",
                    HistoryTraversalFailureReason::PreviewIsolationViolation.as_str(),
                    detail
                ));
                self.workspace
                    .graph_runtime
                    .history_recent_failure_reason_bucket =
                    Some(HistoryTraversalFailureReason::PreviewIsolationViolation);
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_TIMELINE_PREVIEW_ISOLATION_VIOLATION,
                    latency_us: 0,
                });
            }
            GraphIntent::HistoryTimelineReplayStarted => {
                self.workspace.graph_runtime.history_replay_in_progress = true;
                self.workspace.graph_runtime.history_replay_cursor = Some(0);
                self.workspace.graph_runtime.history_replay_total_steps = None;
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_STARTED,
                    latency_us: 0,
                });
            }
            GraphIntent::HistoryTimelineReplaySetTotal { total_steps } => {
                self.workspace.graph_runtime.history_replay_total_steps = Some(total_steps);
                let next_cursor = self
                    .workspace
                    .graph_runtime
                    .history_replay_cursor
                    .unwrap_or(0)
                    .min(total_steps);
                self.workspace.graph_runtime.history_replay_cursor = Some(next_cursor);
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
            }
            GraphIntent::HistoryTimelineReplayAdvance { steps } => {
                let total_steps = self
                    .workspace
                    .graph_runtime
                    .history_replay_total_steps
                    .unwrap_or(0);
                if total_steps == 0 {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::ReplayFailed,
                        "replay advance requested without total steps",
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED,
                        latency_us: 0,
                    });
                    return;
                }

                let was_running = self.workspace.graph_runtime.history_replay_in_progress;
                self.workspace.graph_runtime.history_replay_in_progress = true;
                if !was_running {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_STARTED,
                        latency_us: 0,
                    });
                }

                let current_cursor = self
                    .workspace
                    .graph_runtime
                    .history_replay_cursor
                    .unwrap_or(0);
                let next_cursor = current_cursor.saturating_add(steps).min(total_steps);

                if let Err(err) = self.replay_history_preview_cursor(next_cursor, total_steps) {
                    self.workspace.graph_runtime.history_replay_in_progress = false;
                    self.record_history_failure(
                        HistoryTraversalFailureReason::ReplayFailed,
                        format!("replay advance failed: {err}"),
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED,
                        latency_us: 0,
                    });
                    return;
                }

                self.workspace.graph_runtime.history_replay_cursor = Some(next_cursor);
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());

                if next_cursor >= total_steps {
                    self.workspace.graph_runtime.history_replay_in_progress = false;
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_SUCCEEDED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::HistoryTimelineReplayReset => {
                self.workspace.graph_runtime.history_replay_in_progress = false;
                if self
                    .workspace
                    .graph_runtime
                    .history_replay_total_steps
                    .is_some()
                {
                    self.workspace.graph_runtime.history_replay_cursor = Some(0);
                } else {
                    self.workspace.graph_runtime.history_replay_cursor = None;
                }
                if let Some(snapshot) = self
                    .workspace
                    .graph_runtime
                    .history_preview_live_graph_snapshot
                    .as_ref()
                {
                    self.workspace.graph_runtime.history_preview_graph = Some(snapshot.clone());
                }
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
            }
            GraphIntent::HistoryTimelineReplayProgress {
                cursor,
                total_steps,
            } => {
                self.workspace.graph_runtime.history_replay_in_progress = true;
                self.workspace.graph_runtime.history_replay_total_steps = Some(total_steps);
                self.workspace.graph_runtime.history_replay_cursor = Some(cursor.min(total_steps));
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
            }
            GraphIntent::HistoryTimelineReplayFinished { succeeded, error } => {
                self.workspace.graph_runtime.history_replay_in_progress = false;
                if succeeded {
                    if let Some(total_steps) =
                        self.workspace.graph_runtime.history_replay_total_steps
                    {
                        self.workspace.graph_runtime.history_replay_cursor = Some(total_steps);
                    }
                }
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                if succeeded {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_SUCCEEDED,
                        latency_us: 0,
                    });
                } else {
                    let detail = error.unwrap_or_else(|| "unknown replay failure".to_string());
                    self.workspace.graph_runtime.history_last_error = Some(format!(
                        "{}: {}",
                        HistoryTraversalFailureReason::ReplayFailed.as_str(),
                        detail
                    ));
                    self.workspace
                        .graph_runtime
                        .history_recent_failure_reason_bucket =
                        Some(HistoryTraversalFailureReason::ReplayFailed);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED,
                        latency_us: 0,
                    });
                }
            }
            GraphIntent::HistoryTimelineReturnToPresentFailed { detail } => {
                self.workspace
                    .graph_runtime
                    .history_last_return_to_present_result = Some(format!("failed: {detail}"));
                self.workspace.graph_runtime.history_last_error = Some(format!(
                    "{}: {}",
                    HistoryTraversalFailureReason::ReturnToPresentFailed.as_str(),
                    detail
                ));
                self.workspace
                    .graph_runtime
                    .history_recent_failure_reason_bucket =
                    Some(HistoryTraversalFailureReason::ReturnToPresentFailed);
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_TIMELINE_RETURN_TO_PRESENT_FAILED,
                    latency_us: 0,
                });
            }
            _ => unreachable!("non-history intent routed to apply_history_runtime_intent"),
        }
    }

    fn export_history_archive(&mut self, dissolved: bool) {
        let export_kind = if dissolved { "dissolved" } else { "timeline" };
        let Some(store) = self.services.persistence.as_ref() else {
            self.record_history_failure(
                HistoryTraversalFailureReason::PersistenceUnavailable,
                format!("export {export_kind} requested without persistence"),
            );
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED,
                latency_us: 0,
            });
            return;
        };

        let content = if dissolved {
            store.export_dissolved_archive()
        } else {
            store.export_traversal_archive()
        };

        match content {
            Ok(content) => {
                let Some(home_dir) = dirs::home_dir() else {
                    self.record_history_failure(
                        HistoryTraversalFailureReason::HomeDirectoryUnavailable,
                        format!("{export_kind} export home directory unavailable"),
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED,
                        latency_us: 0,
                    });
                    return;
                };

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let filename = if dissolved {
                    format!("graphshell_dissolved_archive_{}.txt", timestamp)
                } else {
                    format!("graphshell_traversal_archive_{}.txt", timestamp)
                };
                let path = home_dir.join(filename);
                if let Err(error) = std::fs::write(&path, content) {
                    log::error!("Failed to export {export_kind} archive: {error}");
                    self.record_history_failure(
                        HistoryTraversalFailureReason::ExportWriteFailed,
                        format!("{export_kind} export write failed: {error}"),
                    );
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED,
                        latency_us: 0,
                    });
                } else {
                    log::info!("Exported {export_kind} archive to {:?}", path);
                }
            }
            Err(error) => {
                log::error!("Failed to export {export_kind} archive: {error}");
                self.record_history_failure(
                    HistoryTraversalFailureReason::ExportReadFailed,
                    format!("{export_kind} export read failed: {error}"),
                );
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED,
                    latency_us: 0,
                });
            }
        }
    }
}
