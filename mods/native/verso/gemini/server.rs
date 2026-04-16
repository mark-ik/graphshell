/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gemini capsule server — serves Graphshell content as `text/gemini`.
//!
//! Architecture:
//! - TCP listener on a configurable port (default 1965).
//! - TLS via a self-signed certificate generated at startup with `rcgen`.
//!   The cert is ephemeral (regenerated on each start); TOFU clients will
//!   re-pin on restart. Persistent cert storage is a follow-on improvement.
//! - Each accepted connection is handled in its own tokio task.
//! - Content routing maps request paths to [`ServedNode`] entries.
//! - Access control: public routes open; `TrustedPeers` routes require a
//!   Gemini client certificate (enforcement is a follow-on improvement).
//!
//! Gemini protocol summary (gemini://gemini.circumlunar.space/docs/spec.gmi):
//! - Client sends:  `<URL>\r\n`
//! - Server sends:  `<STATUS> <META>\r\n[<BODY>]`
//! - Status codes used here:
//!   - `20 text/gemini` — success
//!   - `51 Not Found`   — unknown path
//!   - `59 Bad Request` — malformed request

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use rcgen::{CertificateParams, KeyPair};
use rustls::ServerConfig;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use uuid::Uuid;

use crate::model::archive::ArchivePrivacyClass;
use middlenet_engine::document::{SimpleBlock, SimpleDocument};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for the capsule server.
#[derive(Debug, Clone)]
pub struct GeminiServerConfig {
    /// TCP port to listen on. Defaults to 1965 (IANA-assigned Gemini port).
    pub port: u16,
    /// Hostname to advertise in the index page and self-reference links.
    pub hostname: String,
}

impl Default for GeminiServerConfig {
    fn default() -> Self {
        Self {
            port: 1965,
            hostname: "localhost".to_string(),
        }
    }
}

/// A node registered for serving via the capsule server.
#[derive(Debug, Clone)]
pub struct ServedNode {
    pub node_id: Uuid,
    /// Human-readable title (used in the index page).
    pub title: String,
    /// Privacy class controls whether a client certificate is required.
    pub privacy_class: ArchivePrivacyClass,
    /// Pre-rendered `text/gemini` content for this node.
    pub gemini_content: String,
}

/// Handle to a running [`GeminiCapsuleServer`].
///
/// Dropping this handle sends a shutdown signal to the server task.
pub struct GeminiServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub bound_addr: SocketAddr,
}

impl GeminiServerHandle {
    /// Gracefully stop the server.
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Content registry shared between the server accept loop and the main thread.
///
/// The main thread adds/removes [`ServedNode`]s; the accept loop reads them
/// per-request. Protected by `RwLock` for concurrent read access.
#[derive(Debug, Default, Clone)]
pub struct CapsuleRegistry {
    inner: Arc<RwLock<HashMap<Uuid, ServedNode>>>,
}

impl CapsuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, node: ServedNode) {
        self.inner.write().unwrap().insert(node.node_id, node);
    }

    pub fn unregister(&self, node_id: Uuid) {
        self.inner.write().unwrap().remove(&node_id);
    }

    pub fn get(&self, node_id: Uuid) -> Option<ServedNode> {
        self.inner.read().unwrap().get(&node_id).cloned()
    }

    pub fn all(&self) -> Vec<ServedNode> {
        self.inner.read().unwrap().values().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Gemini capsule server.
pub struct GeminiCapsuleServer {
    config: GeminiServerConfig,
    pub registry: CapsuleRegistry,
}

impl GeminiCapsuleServer {
    pub fn new(config: GeminiServerConfig) -> Self {
        Self {
            config,
            registry: CapsuleRegistry::new(),
        }
    }

    /// Construct a server that shares an existing [`CapsuleRegistry`].
    ///
    /// Used when the registry is managed externally (e.g., by the
    /// registries runtime) so that node registration and the server
    /// accept loop share the same content store.
    pub fn new_with_registry(config: GeminiServerConfig, registry: CapsuleRegistry) -> Self {
        Self { config, registry }
    }

    /// Start the server on a tokio runtime.
    ///
    /// Returns a [`GeminiServerHandle`] that can be used to stop the server
    /// and observe the actual bound address (useful when port 0 is requested
    /// for tests).
    pub async fn start(self) -> Result<GeminiServerHandle, String> {
        let tls_config = build_tls_config(&self.config.hostname)
            .map_err(|e| format!("gemini server TLS setup failed: {e}"))?;

        let acceptor = TlsAcceptor::from(Arc::new(tls_config));
        let addr: SocketAddr = format!("0.0.0.0:{}", self.config.port)
            .parse()
            .map_err(|e| format!("invalid address: {e}"))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| format!("gemini server bind failed on {addr}: {e}"))?;

        let bound_addr = listener
            .local_addr()
            .map_err(|e| format!("could not get bound address: {e}"))?;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let registry = self.registry.clone();
        let hostname = self.config.hostname.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, peer_addr)) => {
                                let acceptor = acceptor.clone();
                                let registry = registry.clone();
                                let hostname = hostname.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(
                                        stream, peer_addr, acceptor, registry, hostname,
                                    ).await {
                                        log::debug!("gemini: connection error from {peer_addr}: {e}");
                                    }
                                });
                            }
                            Err(e) => {
                                log::warn!("gemini: accept error: {e}");
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        log::info!("gemini: server shutdown");
                        break;
                    }
                }
            }
        });

        log::info!("gemini: capsule server listening on {bound_addr}");

        Ok(GeminiServerHandle {
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
    acceptor: TlsAcceptor,
    registry: CapsuleRegistry,
    hostname: String,
) -> Result<(), String> {
    let tls_stream = acceptor
        .accept(stream)
        .await
        .map_err(|e| format!("TLS handshake failed: {e}"))?;

    let (reader, mut writer) = tokio::io::split(tls_stream);
    let mut buf_reader = BufReader::new(reader);

    // Read the request line: `<URL>\r\n`
    let mut request_line = String::new();
    buf_reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    let url = request_line.trim();
    if url.is_empty() {
        send_response(&mut writer, "59 Bad Request\r\n", None).await?;
        return Ok(());
    }

    log::debug!("gemini: request from {peer_addr}: {url}");

    let path = extract_path(url, &hostname);
    let response = route_request(&path, &registry, &hostname);
    send_response(&mut writer, &response.header, response.body.as_deref()).await?;
    Ok(())
}

struct GeminiResponse {
    header: String,
    body: Option<String>,
}

fn route_request(path: &str, registry: &CapsuleRegistry, hostname: &str) -> GeminiResponse {
    match path {
        "/" | "" => serve_index(registry, hostname),
        p if p.starts_with("/node/") => {
            let id_str = &p[6..];
            match Uuid::parse_str(id_str) {
                Ok(id) => match registry.get(id) {
                    Some(node) => serve_node(&node),
                    None => not_found(),
                },
                Err(_) => GeminiResponse {
                    header: "59 Bad Request\r\n".to_string(),
                    body: None,
                },
            }
        }
        _ => not_found(),
    }
}

fn serve_index(registry: &CapsuleRegistry, hostname: &str) -> GeminiResponse {
    let mut doc_blocks = vec![
        SimpleBlock::Heading {
            level: 1,
            text: format!("{hostname} — Graphshell Capsule"),
        },
        SimpleBlock::Paragraph("This capsule is served by Graphshell.".to_string()),
    ];

    let nodes = registry.all();
    if nodes.is_empty() {
        doc_blocks.push(SimpleBlock::Paragraph(
            "No nodes are currently shared from this capsule.".to_string(),
        ));
    } else {
        doc_blocks.push(SimpleBlock::Heading {
            level: 2,
            text: "Shared nodes".to_string(),
        });
        for node in &nodes {
            doc_blocks.push(SimpleBlock::Link {
                text: node.title.clone(),
                href: format!("gemini://{hostname}/node/{}", node.node_id),
            });
        }
    }

    let body = SimpleDocument::Blocks(doc_blocks).to_gemini();
    GeminiResponse {
        header: "20 text/gemini\r\n".to_string(),
        body: Some(body),
    }
}

fn serve_node(node: &ServedNode) -> GeminiResponse {
    // TrustedPeers and LocalPrivate gating would check the client cert here.
    // For now we serve all registered nodes; the caller controls what gets
    // registered (LocalPrivate nodes should never be passed to `register()`).
    GeminiResponse {
        header: "20 text/gemini\r\n".to_string(),
        body: Some(node.gemini_content.clone()),
    }
}

fn not_found() -> GeminiResponse {
    GeminiResponse {
        header: "51 Not Found\r\n".to_string(),
        body: None,
    }
}

async fn send_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    header: &str,
    body: Option<&str>,
) -> Result<(), String> {
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| format!("write error: {e}"))?;
    if let Some(b) = body {
        writer
            .write_all(b.as_bytes())
            .await
            .map_err(|e| format!("write error: {e}"))?;
    }
    writer
        .flush()
        .await
        .map_err(|e| format!("flush error: {e}"))?;
    Ok(())
}

/// Extract the path component from a Gemini URL for this host.
///
/// Handles both full URLs (`gemini://host/path`) and bare paths (`/path`).
fn extract_path(url: &str, hostname: &str) -> String {
    // Full URL
    if let Some(rest) = url.strip_prefix("gemini://") {
        // Strip host[:port]
        let path_start = rest.find('/').map(|i| i + 1).unwrap_or(rest.len());
        let path = &rest[path_start..];
        let path = if path.is_empty() { "/" } else { path };
        // Strip query string
        let path = path.split('?').next().unwrap_or(path);
        format!("/{}", path.trim_start_matches('/'))
    } else if url.starts_with('/') {
        url.split('?').next().unwrap_or(url).to_string()
    } else {
        // Fallback: treat as root
        let _ = hostname;
        "/".to_string()
    }
}

// ---------------------------------------------------------------------------
// TLS setup
// ---------------------------------------------------------------------------

/// Build a `rustls::ServerConfig` with a self-signed Ed25519 certificate.
///
/// The certificate is ephemeral — generated fresh on each server start.
/// TOFU clients will re-pin after a restart, which is acceptable for a
/// local capsule server. Persistent cert storage is a follow-on improvement.
fn build_tls_config(hostname: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let key_pair = KeyPair::generate()?;

    let mut params = CertificateParams::new(vec![hostname.to_string()])?;
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2099, 12, 31);

    let cert = params.self_signed(&key_pair)?;

    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(key_pair.serialize_der())
        .map_err(|e| format!("key DER error: {e}"))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    Ok(config)
}
