/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gopher capsule server (RFC 1436).
//!
//! Protocol summary:
//! - Plain TCP, no TLS. Default port 70.
//! - Client sends: `<selector>\r\n` (empty selector = root menu)
//! - Server responds with the menu or file content, then closes.
//! - Menu lines: `<type><display>\t<selector>\t<host>\t<port>\r\n`
//! - Menu ends with `.\r\n`
//!
//! Content is serialized from [`SimpleDocument`] to Gophermap on demand.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::model::archive::ArchivePrivacyClass;
use crate::mods::native::verso::gemini::SimpleDocument;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GopherServerConfig {
    pub port: u16,
    pub hostname: String,
}

impl Default for GopherServerConfig {
    fn default() -> Self {
        Self {
            port: 70,
            hostname: "localhost".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GopherServedNode {
    pub node_id: Uuid,
    pub title: String,
    pub privacy_class: ArchivePrivacyClass,
    /// Pre-serialized Gophermap content for this node (type `0` text file).
    pub gophermap_content: String,
}

pub struct GopherServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub bound_addr: SocketAddr,
}

impl GopherServerHandle {
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }
}

#[derive(Debug, Default, Clone)]
pub struct GopherRegistry {
    inner: Arc<RwLock<HashMap<Uuid, GopherServedNode>>>,
}

impl GopherRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, node: GopherServedNode) {
        self.inner.write().unwrap().insert(node.node_id, node);
    }

    pub fn unregister(&self, node_id: Uuid) {
        self.inner.write().unwrap().remove(&node_id);
    }

    pub fn get(&self, node_id: Uuid) -> Option<GopherServedNode> {
        self.inner.read().unwrap().get(&node_id).cloned()
    }

    pub fn all(&self) -> Vec<GopherServedNode> {
        self.inner.read().unwrap().values().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

pub struct GopherCapsuleServer {
    config: GopherServerConfig,
    pub registry: GopherRegistry,
}

impl GopherCapsuleServer {
    pub fn new(config: GopherServerConfig) -> Self {
        Self {
            config,
            registry: GopherRegistry::new(),
        }
    }

    pub fn new_with_registry(config: GopherServerConfig, registry: GopherRegistry) -> Self {
        Self { config, registry }
    }

    pub async fn start(self) -> Result<GopherServerHandle, String> {
        let addr: SocketAddr = format!("0.0.0.0:{}", self.config.port)
            .parse()
            .map_err(|e| format!("invalid address: {e}"))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| format!("gopher server bind failed on {addr}: {e}"))?;

        let bound_addr = listener
            .local_addr()
            .map_err(|e| format!("could not get bound address: {e}"))?;

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let registry = self.registry.clone();
        let hostname = self.config.hostname.clone();
        let port = self.config.port;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, peer_addr)) => {
                                let registry = registry.clone();
                                let hostname = hostname.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(
                                        stream, peer_addr, registry, hostname, port,
                                    ).await {
                                        log::debug!("gopher: connection error from {peer_addr}: {e}");
                                    }
                                });
                            }
                            Err(e) => log::warn!("gopher: accept error: {e}"),
                        }
                    }
                    _ = &mut shutdown_rx => {
                        log::info!("gopher: server shutdown");
                        break;
                    }
                }
            }
        });

        log::info!("gopher: capsule server listening on {bound_addr}");
        Ok(GopherServerHandle {
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
    registry: GopherRegistry,
    hostname: String,
    port: u16,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let mut selector = String::new();
    buf_reader
        .read_line(&mut selector)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    let selector = selector.trim().to_string();
    log::debug!("gopher: request from {peer_addr}: selector={selector:?}");

    let response = route_selector(&selector, &registry, &hostname, port);
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

fn route_selector(selector: &str, registry: &GopherRegistry, hostname: &str, port: u16) -> String {
    match selector {
        "" | "/" => serve_root_menu(registry, hostname, port),
        s if s.starts_with("/node/") => {
            let id_str = &s[6..];
            match Uuid::parse_str(id_str) {
                Ok(id) => match registry.get(id) {
                    Some(node) => node.gophermap_content.clone(),
                    None => "3Not Found\t/\t\t0\r\n.\r\n".to_string(),
                },
                Err(_) => "3Invalid selector\t/\t\t0\r\n.\r\n".to_string(),
            }
        }
        _ => "3Not Found\t/\t\t0\r\n.\r\n".to_string(),
    }
}

fn serve_root_menu(registry: &GopherRegistry, hostname: &str, port: u16) -> String {
    let index_doc = SimpleDocument::Blocks(vec![
        crate::mods::native::verso::gemini::SimpleBlock::Heading {
            level: 1,
            text: format!("{hostname} — Graphshell Gopherspace"),
        },
        crate::mods::native::verso::gemini::SimpleBlock::Paragraph(
            "This gopherspace is served by Graphshell.".to_string(),
        ),
    ]);
    let mut out = index_doc.to_gophermap(hostname, port);
    // Remove trailing ".\r\n" to append node entries, then re-add it
    if out.ends_with(".\r\n") {
        out.truncate(out.len() - 3);
    }

    let nodes = registry.all();
    if nodes.is_empty() {
        out.push_str("iNo nodes are currently shared from this gopherspace.\tfake\tfake\t70\r\n");
    } else {
        out.push_str("i\tfake\tfake\t70\r\n");
        out.push_str("iShared nodes:\tfake\tfake\t70\r\n");
        for node in &nodes {
            let safe_title = node.title.replace('\t', " ");
            let selector = format!("/node/{}", node.node_id);
            out.push_str(&format!(
                "0{safe_title}\t{selector}\t{hostname}\t{port}\r\n"
            ));
        }
    }
    out.push_str(".\r\n");
    out
}
