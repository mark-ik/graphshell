/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Finger server (RFC 1288).
//!
//! Protocol summary:
//! - Plain TCP, no TLS. Default port 79.
//! - Client sends: `[/W] [<query>]\r\n`
//! - Server responds with plain text, then closes the connection.
//!
//! In Graphshell, Finger serves personal profile content — a
//! `SimpleDocument` serialized to plain text. Queries map to named
//! profiles (e.g., `""` or `"graphshell"` returns the default public
//! profile; a node title or UUID returns that node's Finger card).
//!
//! This is the "inbuilt tumblr" layer: a human-readable, selectively
//! published identity representation reachable with any Finger client.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::model::archive::ArchivePrivacyClass;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FingerServerConfig {
    pub port: u16,
    /// The default query name served when client sends an empty request.
    /// Typically the user's handle or hostname.
    pub default_query: String,
}

impl Default for FingerServerConfig {
    fn default() -> Self {
        Self {
            port: 79,
            default_query: "graphshell".to_string(),
        }
    }
}

/// A named Finger profile entry.
///
/// The `query_name` is the key clients use to request this profile
/// (e.g., `finger user@host` sends `user\r\n`). The empty string is
/// the default profile returned for bare `finger @host` queries.
#[derive(Debug, Clone)]
pub struct FingerProfile {
    /// Query name (empty string = default profile).
    pub query_name: String,
    pub privacy_class: ArchivePrivacyClass,
    /// Pre-serialized plain text content for this profile.
    pub finger_text: String,
}

pub struct FingerServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub bound_addr: SocketAddr,
}

impl FingerServerHandle {
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }
}

#[derive(Debug, Default, Clone)]
pub struct FingerRegistry {
    /// Key: query_name (empty string = default profile)
    inner: Arc<RwLock<HashMap<String, FingerProfile>>>,
}

impl FingerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, profile: FingerProfile) {
        self.inner
            .write()
            .unwrap()
            .insert(profile.query_name.clone(), profile);
    }

    pub fn unregister(&self, query_name: &str) {
        self.inner.write().unwrap().remove(query_name);
    }

    pub fn get(&self, query_name: &str) -> Option<FingerProfile> {
        self.inner.read().unwrap().get(query_name).cloned()
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

pub struct FingerServer {
    config: FingerServerConfig,
    pub registry: FingerRegistry,
}

impl FingerServer {
    pub fn new(config: FingerServerConfig) -> Self {
        Self {
            config,
            registry: FingerRegistry::new(),
        }
    }

    pub fn new_with_registry(config: FingerServerConfig, registry: FingerRegistry) -> Self {
        Self { config, registry }
    }

    pub async fn start(self) -> Result<FingerServerHandle, String> {
        let addr: SocketAddr = format!("0.0.0.0:{}", self.config.port)
            .parse()
            .map_err(|e| format!("invalid address: {e}"))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| format!("finger server bind failed on {addr}: {e}"))?;

        let bound_addr = listener
            .local_addr()
            .map_err(|e| format!("could not get bound address: {e}"))?;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let registry = self.registry.clone();
        let default_query = self.config.default_query.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, peer_addr)) => {
                                let registry = registry.clone();
                                let default_query = default_query.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(
                                        stream, peer_addr, registry, default_query,
                                    ).await {
                                        log::debug!("finger: connection error from {peer_addr}: {e}");
                                    }
                                });
                            }
                            Err(e) => log::warn!("finger: accept error: {e}"),
                        }
                    }
                    _ = &mut shutdown_rx => {
                        log::info!("finger: server shutdown");
                        break;
                    }
                }
            }
        });

        log::info!("finger: server listening on {bound_addr}");
        Ok(FingerServerHandle {
            shutdown_tx,
            bound_addr,
        })
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    registry: FingerRegistry,
    default_query: String,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let mut line = String::new();
    buf_reader
        .read_line(&mut line)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    // Strip optional `/W` (verbose) flag and trailing whitespace
    let query = line.trim().trim_start_matches("/W").trim().to_string();
    let query = if query.is_empty() {
        default_query.clone()
    } else {
        query
    };

    log::debug!("finger: request from {peer_addr}: query={query:?}");

    let response = match registry.get(&query) {
        Some(profile) => profile.finger_text.clone(),
        None => match registry.get("") {
            // Fall back to default profile for unknown queries
            Some(default) => format!(
                "No profile found for '{}'\n\nDefault profile:\n\n{}",
                query, default.finger_text
            ),
            None => format!("No profile found for '{query}'\n"),
        },
    };

    writer
        .write_all(response.as_bytes())
        .await
        .map_err(|e| format!("write error: {e}"))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("flush error: {e}"))?;
    Ok(())
}
