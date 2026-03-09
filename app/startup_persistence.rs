use super::*;

impl GraphBrowserApp {
    pub(crate) fn recover_graph_for_startup(data_dir: PathBuf) -> (Graph, Option<GraphStore>) {
        match Self::open_store_for_startup(data_dir) {
            Ok(store) => {
                let graph = match store.recover() {
                    Some(recovered) => {
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED,
                            latency_us: 1,
                        });
                        recovered
                    }
                    None => {
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_PERSISTENCE_RECOVER_FAILED,
                            latency_us: 1,
                        });
                        warn!("Failed to recover graph store");
                        Graph::new()
                    }
                };
                (graph, Some(store))
            }
            Err(error) => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                    latency_us: 1,
                });
                warn!("Failed to open graph store: {error}");
                (Graph::new(), None)
            }
        }
    }

    fn open_store_for_startup(data_dir: PathBuf) -> Result<GraphStore, String> {
        #[cfg(test)]
        {
            return GraphStore::open(data_dir).map_err(|error| error.to_string());
        }

        #[cfg(not(test))]
        {
            let start = Instant::now();
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED,
                byte_len: data_dir.to_string_lossy().len(),
            });
            let timeout_ms = Self::startup_persistence_timeout_ms();
            let (tx, rx) = mpsc::channel();

            std::thread::Builder::new()
                .name("graphstore-open".to_string())
                .spawn(move || {
                    let _ = tx.send(GraphStore::open(data_dir));
                })
                .map_err(|error| format!("failed to spawn persistence-open thread: {error}"))?;

            if timeout_ms == 0 {
                let result = rx.recv().map_err(|_| {
                    "persistence-open worker disconnected before sending result".to_string()
                })?;

                match &result {
                    Ok(_) => emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
                        latency_us: start.elapsed().as_micros() as u64,
                    }),
                    Err(_) => emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                        latency_us: start.elapsed().as_micros() as u64,
                    }),
                }

                return result.map_err(|error| error.to_string());
            }

            match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
                Ok(result) => {
                    match &result {
                        Ok(_) => emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
                            latency_us: start.elapsed().as_micros() as u64,
                        }),
                        Err(_) => emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                            latency_us: start.elapsed().as_micros() as u64,
                        }),
                    }
                    result.map_err(|error| error.to_string())
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT,
                        latency_us: start.elapsed().as_micros() as u64,
                    });
                    Err(format!(
                        "startup persistence open timed out after {}ms; continuing without persistence",
                        timeout_ms
                    ))
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                        latency_us: start.elapsed().as_micros() as u64,
                    });
                    Err("persistence-open worker disconnected before sending result".to_string())
                }
            }
        }
    }

    fn startup_persistence_timeout_ms() -> u64 {
        env::var("GRAPHSHELL_PERSISTENCE_OPEN_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(Self::STARTUP_PERSISTENCE_OPEN_TIMEOUT_MS)
    }
}