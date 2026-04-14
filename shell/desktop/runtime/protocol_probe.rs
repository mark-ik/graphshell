/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::time::Duration;

use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContentTypeProbeResult {
    pub(crate) mime_hint: Option<String>,
}

pub(crate) struct ContentTypeProber;

impl ContentTypeProber {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn probe(
        &self,
        url: String,
        cancel: CancellationToken,
    ) -> Option<ContentTypeProbeResult> {
        if cancel.is_cancelled() {
            return None;
        }

        tokio::select! {
            _ = cancel.cancelled() => None,
            result = tokio::task::spawn_blocking(move || probe_content_type(&url)) => {
                if cancel.is_cancelled() {
                    return None;
                }
                result.ok().flatten().map(|mime_hint| ContentTypeProbeResult { mime_hint })
            }
        }
    }
}

impl Default for ContentTypeProber {
    fn default() -> Self {
        Self::new()
    }
}

fn probe_content_type(url: &str) -> Option<Option<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let response = client.head(url).send().ok()?;
    let header = response.headers().get(reqwest::header::CONTENT_TYPE)?;
    let raw = header.to_str().ok()?.trim();
    if raw.is_empty() {
        return Some(None);
    }

    let mime_hint = raw
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    Some(mime_hint)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn spawn_head_server(content_type: &'static str) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose address");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Type: {content_type}\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        format!("http://{}", address)
    }

    #[tokio::test]
    async fn probe_returns_content_type_header_value() {
        let url = spawn_head_server("text/csv; charset=utf-8");
        let result = ContentTypeProber::default()
            .probe(url, CancellationToken::new())
            .await
            .expect("probe should complete");

        assert_eq!(result.mime_hint.as_deref(), Some("text/csv"));
    }

    #[tokio::test]
    async fn probe_honors_pre_cancelled_token() {
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = ContentTypeProber::default()
            .probe("http://127.0.0.1:9".to_string(), cancel)
            .await;

        assert!(result.is_none());
    }
}

