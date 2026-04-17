/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error, SignatureScheme, StreamOwned,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use middlenet_engine::source::{MiddleNetContentKind, MiddleNetSource};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const SPARTAN_MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFetch {
    pub body: String,
    pub content_kind_override: Option<MiddleNetContentKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitanUploadOutcome {
    pub status: u16,
    pub meta: String,
    pub body: String,
}

impl RemoteFetch {
    fn new(body: String) -> Self {
        Self {
            body,
            content_kind_override: None,
        }
    }

    fn with_content_kind(
        body: String,
        content_kind_override: Option<MiddleNetContentKind>,
    ) -> Self {
        Self {
            body,
            content_kind_override,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct GeminiKnownHostRecord {
    authority: String,
    fingerprint_sha256: String,
}

#[derive(Debug, Clone)]
struct GeminiKnownHostsStore {
    path: Option<PathBuf>,
    records: Arc<RwLock<HashMap<String, GeminiKnownHostRecord>>>,
}

impl GeminiKnownHostsStore {
    fn load_default() -> Self {
        let path = gemini_known_hosts_path();
        let records = path
            .as_ref()
            .and_then(|path| load_known_hosts_from_path(path).ok())
            .unwrap_or_default();
        Self {
            path,
            records: Arc::new(RwLock::new(records)),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn new_for_tests(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn remember_or_verify(
        &self,
        authority: &str,
        certificate: &CertificateDer<'_>,
    ) -> Result<(), String> {
        let fingerprint = sha256_hex(certificate.as_ref());
        let mut records = self
            .records
            .write()
            .expect("gemini known-hosts lock poisoned");

        match records.get(authority) {
            Some(existing) if existing.fingerprint_sha256 == fingerprint => Ok(()),
            Some(existing) => Err(format!(
                "Gemini certificate changed for {authority}. Stored fingerprint {stored}, received {received}.",
                stored = existing.fingerprint_sha256,
                received = fingerprint,
            )),
            None => {
                records.insert(
                    authority.to_string(),
                    GeminiKnownHostRecord {
                        authority: authority.to_string(),
                        fingerprint_sha256: fingerprint,
                    },
                );
                drop(records);
                self.persist();
                Ok(())
            }
        }
    }

    fn persist(&self) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut records = self
            .records
            .read()
            .expect("gemini known-hosts lock poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.authority.cmp(&right.authority));
        let Ok(content) = serde_json::to_string_pretty(&records) else {
            return;
        };
        let _ = fs::write(path, content);
    }
}

#[derive(Debug)]
struct GeminiTofuVerifier {
    authority: String,
    known_hosts: GeminiKnownHostsStore,
    supported_algs: WebPkiSupportedAlgorithms,
}

impl GeminiTofuVerifier {
    fn new(authority: String, known_hosts: GeminiKnownHostsStore) -> Self {
        let supported_algs =
            rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms;
        Self {
            authority,
            known_hosts,
            supported_algs,
        }
    }
}

impl ServerCertVerifier for GeminiTofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        self.known_hosts
            .remember_or_verify(&self.authority, end_entity)
            .map_err(Error::General)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.supported_algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.supported_algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported_algs.supported_schemes()
    }
}

pub fn fetch_remote_text(source: &MiddleNetSource) -> Result<RemoteFetch, String> {
    let canonical_uri = source
        .canonical_uri
        .as_deref()
        .ok_or_else(|| "MiddleNet transport requires a canonical URI.".to_string())?;
    let parsed = url::Url::parse(canonical_uri)
        .map_err(|error| format!("Invalid MiddleNet URI '{canonical_uri}': {error}"))?;

    match parsed.scheme() {
        "http" | "https" => fetch_http_text(parsed.as_str()),
        "gopher" => fetch_gopher_text(&parsed),
        "finger" => fetch_finger_text(&parsed),
        "gemini" => fetch_gemini_text(&parsed),
        "spartan" => fetch_spartan_text(&parsed),
        "misfin" => Err(
            "Misfin transport is still pending because the MiddleNet lane only has document fetching wired so far, not the message-oriented TLS flow Misfin needs."
                .to_string(),
        ),
        scheme => Err(format!(
            "MiddleNet transport does not support remote '{scheme}' fetching yet."
        )),
    }
}

fn fetch_http_text(url: &str) -> Result<RemoteFetch, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(IO_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to build HTTP client: {error}"))?
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("HTTP fetch failed for '{url}': {error}"))?;

    let content_kind_override = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(content_kind_from_content_type);
    let body = response
        .text()
        .map_err(|error| format!("HTTP response body decode failed for '{url}': {error}"))?;

    Ok(RemoteFetch::with_content_kind(body, content_kind_override))
}

fn fetch_gopher_text(url: &url::Url) -> Result<RemoteFetch, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Gopher URL is missing a host.".to_string())?;
    let port = url.port().unwrap_or(70);
    let selector = gopher_selector(url);
    let mut stream = connect(host, port)?;
    stream
        .write_all(format!("{selector}\r\n").as_bytes())
        .map_err(|error| format!("Gopher request write failed: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("Gopher request flush failed: {error}"))?;

    let mut body = Vec::new();
    stream
        .read_to_end(&mut body)
        .map_err(|error| format!("Gopher response read failed: {error}"))?;
    Ok(RemoteFetch::new(
        String::from_utf8_lossy(&body).into_owned(),
    ))
}

fn fetch_finger_text(url: &url::Url) -> Result<RemoteFetch, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Finger URL is missing a host.".to_string())?;
    let port = url.port().unwrap_or(79);
    let query = finger_query(url);
    let mut stream = connect(host, port)?;
    stream
        .write_all(format!("{query}\r\n").as_bytes())
        .map_err(|error| format!("Finger request write failed: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("Finger request flush failed: {error}"))?;

    let mut body = String::new();
    stream
        .read_to_string(&mut body)
        .map_err(|error| format!("Finger response read failed: {error}"))?;
    Ok(RemoteFetch::new(body))
}

pub fn titan_upload(
    url: &url::Url,
    content: &[u8],
    mime: Option<&str>,
    token: Option<&str>,
) -> Result<TitanUploadOutcome, String> {
    let known_hosts = GeminiKnownHostsStore::load_default();
    titan_upload_with_store(url, content, mime, token, &known_hosts)
}

#[cfg(any(test, feature = "test-support"))]
pub fn titan_upload_for_tests(
    url: &url::Url,
    content: &[u8],
    mime: Option<&str>,
    token: Option<&str>,
    known_hosts_path: &Path,
) -> Result<TitanUploadOutcome, String> {
    let known_hosts = GeminiKnownHostsStore::new_for_tests(known_hosts_path.to_path_buf());
    titan_upload_with_store(url, content, mime, token, &known_hosts)
}

fn titan_upload_with_store(
    url: &url::Url,
    content: &[u8],
    mime: Option<&str>,
    token: Option<&str>,
    known_hosts: &GeminiKnownHostsStore,
) -> Result<TitanUploadOutcome, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Titan URL is missing a host.".to_string())?;
    let port = url.port().unwrap_or(1965);
    let authority = format!("{}:{port}", host.to_ascii_lowercase());
    let request_url = compose_titan_request_target(url, content.len(), mime, token)?;
    let stream = connect(host, port)?;

    let verifier = Arc::new(GeminiTofuVerifier::new(authority, known_hosts.clone()));
    let client_config =
        ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
            .with_protocol_versions(rustls::DEFAULT_VERSIONS)
            .expect("rustls default protocol versions should be valid for Titan client")
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
    let server_name = server_name_for_host(host)?;
    let connection = ClientConnection::new(Arc::new(client_config), server_name)
        .map_err(|error| format!("Titan TLS client setup failed: {error}"))?;
    let mut tls_stream = StreamOwned::new(connection, stream);

    tls_stream
        .write_all(format!("{request_url}\r\n").as_bytes())
        .map_err(|error| format!("Titan request write failed: {error}"))?;
    if !content.is_empty() {
        tls_stream
            .write_all(content)
            .map_err(|error| format!("Titan content write failed: {error}"))?;
    }
    tls_stream
        .flush()
        .map_err(|error| format!("Titan request flush failed: {error}"))?;

    if tls_stream.conn.peer_certificates().is_none() {
        return Err("Titan TLS handshake completed without a peer certificate.".to_string());
    }

    let mut reader = BufReader::new(tls_stream);
    let (status, meta, body) = read_gemini_style_response(&mut reader, "Titan")?;

    Ok(TitanUploadOutcome { status, meta, body })
}

fn fetch_gemini_text(url: &url::Url) -> Result<RemoteFetch, String> {
    let known_hosts = GeminiKnownHostsStore::load_default();
    fetch_gemini_text_with_store(url, &known_hosts)
}

fn fetch_spartan_text(url: &url::Url) -> Result<RemoteFetch, String> {
    fetch_spartan_text_with_redirects(url, 0)
}

fn fetch_spartan_text_with_redirects(
    url: &url::Url,
    redirect_depth: usize,
) -> Result<RemoteFetch, String> {
    if redirect_depth >= SPARTAN_MAX_REDIRECTS {
        return Err("Spartan redirect limit exceeded.".to_string());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "Spartan URL is missing a host.".to_string())?;
    let port = url.port().unwrap_or(300);
    let request_host = spartan_request_host(url)?;
    let request_path = spartan_request_path(url);
    let request_body = spartan_request_body(url)?;

    let mut stream = connect(host, port)?;
    stream
        .write_all(format!("{request_host} {request_path} {}\r\n", request_body.len()).as_bytes())
        .map_err(|error| format!("Spartan request write failed: {error}"))?;
    if !request_body.is_empty() {
        stream
            .write_all(&request_body)
            .map_err(|error| format!("Spartan request body write failed: {error}"))?;
    }
    stream
        .flush()
        .map_err(|error| format!("Spartan request flush failed: {error}"))?;

    let mut reader = BufReader::new(stream);
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|error| format!("Spartan response header read failed: {error}"))?;
    let (status, meta) = parse_spartan_header(&header)?;

    match status {
        2 => {
            let mut body = Vec::new();
            reader
                .read_to_end(&mut body)
                .map_err(|error| format!("Spartan response body read failed: {error}"))?;
            Ok(RemoteFetch::with_content_kind(
                String::from_utf8_lossy(&body).into_owned(),
                content_kind_from_content_type(&meta),
            ))
        }
        3 => {
            let redirected = spartan_redirect_url(url, &meta)?;
            fetch_spartan_text_with_redirects(&redirected, redirect_depth + 1)
        }
        4 => Err(format!("Spartan client error: {meta}")),
        5 => Err(format!("Spartan server error: {meta}")),
        _ => Err(format!("Unsupported Spartan status {status}: {meta}")),
    }
}

fn fetch_gemini_text_with_store(
    url: &url::Url,
    known_hosts: &GeminiKnownHostsStore,
) -> Result<RemoteFetch, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Gemini URL is missing a host.".to_string())?;
    let port = url.port().unwrap_or(1965);
    let authority = format!("{}:{port}", host.to_ascii_lowercase());
    let stream = connect(host, port)?;

    let verifier = Arc::new(GeminiTofuVerifier::new(authority, known_hosts.clone()));
    let client_config =
        ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
            .with_protocol_versions(rustls::DEFAULT_VERSIONS)
            .expect("rustls default protocol versions should be valid for Gemini client")
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
    let server_name = server_name_for_host(host)?;
    let connection = ClientConnection::new(Arc::new(client_config), server_name)
        .map_err(|error| format!("Gemini TLS client setup failed: {error}"))?;
    let mut tls_stream = StreamOwned::new(connection, stream);

    tls_stream
        .write_all(format!("{}\r\n", url.as_str()).as_bytes())
        .map_err(|error| format!("Gemini request write failed: {error}"))?;
    tls_stream
        .flush()
        .map_err(|error| format!("Gemini request flush failed: {error}"))?;

    if tls_stream.conn.peer_certificates().is_none() {
        return Err("Gemini TLS handshake completed without a peer certificate.".to_string());
    }

    let mut reader = BufReader::new(tls_stream);
    let (status, meta, body) = read_gemini_style_response(&mut reader, "Gemini")?;

    match status {
        20..=29 => Ok(RemoteFetch::with_content_kind(
            body,
            content_kind_from_content_type(&meta),
        )),
        30..=39 => Err(format!(
            "Gemini redirect handling is not wired yet (status {status}, target '{meta}')."
        )),
        10..=19 => Err(format!(
            "Gemini server requested input before content delivery (status {status}, meta '{meta}')."
        )),
        40..=49 => Err(format!("Gemini temporary failure {status}: {meta}")),
        50..=59 => Err(format!("Gemini permanent failure {status}: {meta}")),
        60..=69 => Err(format!(
            "Gemini certificate handling status {status}: {meta}"
        )),
        _ => Err(format!("Unsupported Gemini status {status}: {meta}")),
    }
}

fn read_gemini_style_response<R: Read>(
    reader: &mut BufReader<R>,
    protocol_name: &str,
) -> Result<(u16, String, String), String> {
    let mut response = Vec::new();
    match reader.read_to_end(&mut response) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(error) => {
            return Err(format!("{protocol_name} response read failed: {error}"));
        }
    }

    let Some(header_end) = response.iter().position(|byte| *byte == b'\n') else {
        return Err(format!(
            "{protocol_name} response header read failed: missing line terminator."
        ));
    };

    let header = String::from_utf8_lossy(&response[..=header_end]).into_owned();
    let (status, meta) = parse_gemini_header(&header)?;
    let body = String::from_utf8_lossy(&response[header_end + 1..]).into_owned();

    Ok((status, meta, body))
}

fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    let mut last_error = None;

    for address in resolve_socket_addrs(host, port)? {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(IO_TIMEOUT))
                    .map_err(|error| format!("Failed to configure read timeout: {error}"))?;
                stream
                    .set_write_timeout(Some(IO_TIMEOUT))
                    .map_err(|error| format!("Failed to configure write timeout: {error}"))?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }

    let error = last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "no socket addresses resolved".to_string());

    Err(format!("Connection to {host}:{port} failed: {error}"))
}

fn resolve_socket_addrs(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, String> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("Failed to resolve {host}:{port}: {error}"))?
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        return Err(format!("No socket address resolved for {host}:{port}."));
    }

    Ok(addresses)
}

fn server_name_for_host(host: &str) -> Result<ServerName<'static>, String> {
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(ServerName::IpAddress(address.into()));
    }

    ServerName::try_from(host.to_string())
        .map_err(|error| format!("Invalid Gemini host '{host}': {error}"))
}

fn parse_gemini_header(header: &str) -> Result<(u16, String), String> {
    let trimmed = header.trim_end_matches(['\r', '\n']);
    if trimmed.len() < 2 {
        return Err(
            "Gemini response header was shorter than the required two-digit status code."
                .to_string(),
        );
    }

    let status = trimmed[..2]
        .parse::<u16>()
        .map_err(|error| format!("Invalid Gemini status code '{}': {error}", &trimmed[..2]))?;
    let meta = trimmed[2..].trim_start().to_string();
    Ok((status, meta))
}

fn parse_spartan_header(header: &str) -> Result<(u16, String), String> {
    let trimmed = header.trim_end_matches(['\r', '\n']);
    let Some((status, meta)) = trimmed.split_once(' ') else {
        return Err(
            "Spartan response header was missing the required status/meta separator.".to_string(),
        );
    };
    if status.len() != 1 {
        return Err(format!("Invalid Spartan status code '{status}'."));
    }
    let status = status
        .parse::<u16>()
        .map_err(|error| format!("Invalid Spartan status code '{status}': {error}"))?;
    Ok((status, meta.to_string()))
}

fn compose_titan_request_target(
    url: &url::Url,
    size: usize,
    mime: Option<&str>,
    token: Option<&str>,
) -> Result<String, String> {
    match url.scheme() {
        "gemini" | "titan" => {}
        scheme => {
            return Err(format!(
                "Titan upload requires a gemini:// or titan:// base URL, got '{scheme}'."
            ));
        }
    }

    if let Some(mime) = mime {
        validate_titan_parameter("mime", mime)?;
    }
    if let Some(token) = token {
        validate_titan_parameter("token", token)?;
    }

    let mut target = String::from("titan://");
    if !url.username().is_empty() {
        target.push_str(url.username());
        if let Some(password) = url.password() {
            target.push(':');
            target.push_str(password);
        }
        target.push('@');
    }
    target.push_str(&spartan_request_host(url)?);
    if let Some(port) = url.port() {
        target.push(':');
        target.push_str(&port.to_string());
    }

    let path = &url[url::Position::BeforePath..url::Position::AfterPath];
    if path.is_empty() {
        target.push('/');
    } else {
        target.push_str(path);
    }

    target.push_str(&format!(";size={size}"));
    if let Some(mime) = mime.filter(|mime| !mime.is_empty() && *mime != "text/gemini") {
        target.push_str(";mime=");
        target.push_str(mime);
    }
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        target.push_str(";token=");
        target.push_str(token);
    }

    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    Ok(target)
}

fn validate_titan_parameter(name: &str, value: &str) -> Result<(), String> {
    if value.contains(['\r', '\n', ';', '#']) {
        return Err(format!(
            "Titan {name} parameter contains reserved characters that must be pre-encoded."
        ));
    }
    Ok(())
}

fn content_kind_from_content_type(meta: &str) -> Option<MiddleNetContentKind> {
    let mime = meta
        .split(';')
        .next()
        .unwrap_or(meta)
        .trim()
        .to_ascii_lowercase();

    match mime.as_str() {
        "text/gemini" | "text/x-gemini" | "" => Some(MiddleNetContentKind::GeminiText),
        "text/plain" => Some(MiddleNetContentKind::PlainText),
        "text/markdown" | "text/x-markdown" => Some(MiddleNetContentKind::Markdown),
        "application/rss+xml" => Some(MiddleNetContentKind::Rss),
        "application/atom+xml" => Some(MiddleNetContentKind::Atom),
        "application/feed+json" => Some(MiddleNetContentKind::JsonFeed),
        _ => None,
    }
}

fn gopher_selector(url: &url::Url) -> String {
    let selector = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    match url.query() {
        Some(query) if !query.is_empty() => format!("{selector}\t{query}"),
        _ => selector.to_string(),
    }
}

fn spartan_request_host(url: &url::Url) -> Result<String, String> {
    match url.host() {
        Some(url::Host::Domain(host)) => Ok(host.to_string()),
        Some(url::Host::Ipv4(address)) => Ok(address.to_string()),
        Some(url::Host::Ipv6(address)) => Ok(format!("[{address}]")),
        None => Err("Spartan URL is missing a host.".to_string()),
    }
}

fn spartan_request_path(url: &url::Url) -> &str {
    let path = url.path();
    if path.is_empty() { "/" } else { path }
}

fn spartan_request_body(url: &url::Url) -> Result<Vec<u8>, String> {
    let Some(query) = url.query() else {
        return Ok(Vec::new());
    };
    percent_decode_component(query)
}

fn spartan_redirect_url(url: &url::Url, path: &str) -> Result<url::Url, String> {
    if !path.starts_with('/') {
        return Err(format!(
            "Invalid Spartan redirect target '{path}': redirects must be absolute paths on the same host."
        ));
    }

    let mut redirected = url.clone();
    redirected.set_path(path);
    redirected.set_query(None);
    redirected.set_fragment(None);
    Ok(redirected)
}

fn percent_decode_component(input: &str) -> Result<Vec<u8>, String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(format!(
                    "Invalid percent-encoding in Spartan URL query '{input}'."
                ));
            }
            let high = from_hex_digit(bytes[index + 1])?;
            let low = from_hex_digit(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    Ok(decoded)
}

fn from_hex_digit(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!(
            "Invalid percent-encoded hex digit '{}'.",
            byte as char
        )),
    }
}

fn finger_query(url: &url::Url) -> String {
    if !url.username().is_empty() {
        return url.username().to_string();
    }

    let path = url.path().trim_start_matches('/').trim();
    if path.is_empty() {
        String::new()
    } else {
        path.to_string()
    }
}

#[cfg(not(any(test, feature = "test-support")))]
fn gemini_known_hosts_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("graphshell");
    path.push("gemini_known_hosts.json");
    Some(path)
}

#[cfg(any(test, feature = "test-support"))]
fn gemini_known_hosts_path() -> Option<PathBuf> {
    None
}

fn load_known_hosts_from_path(
    path: &Path,
) -> Result<HashMap<String, GeminiKnownHostRecord>, std::io::Error> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(path)?;
    match serde_json::from_str::<Vec<GeminiKnownHostRecord>>(&content) {
        Ok(records) => Ok(records
            .into_iter()
            .map(|record| (record.authority.clone(), record))
            .collect()),
        Err(error) => {
            log::warn!("gemini known-hosts load failed: {error}; resetting known-hosts store");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, "[]")?;
            Ok(HashMap::new())
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    encode_hex(&digest)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("nibble values must be in 0..=15"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_engine::source::MiddleNetContentKind;
    use rcgen::{CertificateParams, KeyPair};
    use rustls::{ServerConfig, ServerConnection};
    use std::net::TcpListener;
    use std::thread;
    use tempfile::TempDir;

    fn source(uri: &str) -> MiddleNetSource {
        MiddleNetSource::new(MiddleNetContentKind::PlainText).with_uri(uri)
    }

    #[test]
    fn fetch_remote_text_reads_gopher_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut selector = String::new();
            reader.read_line(&mut selector).expect("selector");
            assert_eq!(selector, "/docs\r\n");

            let mut writer = stream;
            writer
                .write_all(b"iHello\tfake\tfake\t70\r\n.\r\n")
                .expect("write");
            writer.flush().expect("flush");
        });

        let fetch = fetch_remote_text(&source(&format!("gopher://127.0.0.1:{port}/docs")))
            .expect("gopher fetch should succeed");

        assert!(fetch.body.contains("Hello"));
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn fetch_remote_text_reads_finger_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut query = String::new();
            reader.read_line(&mut query).expect("query");
            assert_eq!(query, "alice\r\n");

            let mut writer = stream;
            writer.write_all(b"Profile\n").expect("write");
            writer.flush().expect("flush");
        });

        let fetch = fetch_remote_text(&source(&format!("finger://alice@127.0.0.1:{port}")))
            .expect("finger fetch should succeed");

        assert_eq!(fetch.body, "Profile\n");
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn fetch_remote_text_reads_spartan_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("request line");
            assert_eq!(request, "127.0.0.1 /capsule 0\r\n");

            let mut writer = stream;
            writer
                .write_all(b"2 text/gemini\r\n# Spartan\n")
                .expect("write");
            writer.flush().expect("flush");
        });

        let fetch = fetch_remote_text(&source(&format!("spartan://127.0.0.1:{port}/capsule")))
            .expect("spartan fetch should succeed");

        assert_eq!(
            fetch.content_kind_override,
            Some(MiddleNetContentKind::GeminiText)
        );
        assert_eq!(fetch.body, "# Spartan\n");
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn spartan_query_becomes_request_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("request line");
            assert_eq!(request, "127.0.0.1 /search 11\r\n");

            let mut body = [0_u8; 11];
            reader.read_exact(&mut body).expect("request body");
            assert_eq!(&body, b"hello world");

            let mut writer = stream;
            writer
                .write_all(b"2 text/plain\r\nResult\n")
                .expect("write");
            writer.flush().expect("flush");
        });

        let fetch = fetch_remote_text(&source(&format!(
            "spartan://127.0.0.1:{port}/search?hello%20world"
        )))
        .expect("spartan query fetch should succeed");

        assert_eq!(
            fetch.content_kind_override,
            Some(MiddleNetContentKind::PlainText)
        );
        assert_eq!(fetch.body, "Result\n");
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn spartan_redirects_follow_same_host_path() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (first_stream, _) = listener.accept().expect("first accept");
            let mut first_reader = BufReader::new(first_stream.try_clone().expect("clone"));
            let mut first_request = String::new();
            first_reader
                .read_line(&mut first_request)
                .expect("first request line");
            assert_eq!(first_request, "127.0.0.1 / 0\r\n");

            let mut first_writer = first_stream;
            first_writer.write_all(b"3 /next\r\n").expect("redirect");
            first_writer.flush().expect("flush");

            let (second_stream, _) = listener.accept().expect("second accept");
            let mut second_reader = BufReader::new(second_stream.try_clone().expect("clone"));
            let mut second_request = String::new();
            second_reader
                .read_line(&mut second_request)
                .expect("second request line");
            assert_eq!(second_request, "127.0.0.1 /next 0\r\n");

            let mut second_writer = second_stream;
            second_writer
                .write_all(b"2 text/gemini\r\n=> /done Done\n")
                .expect("success");
            second_writer.flush().expect("flush");
        });

        let fetch = fetch_remote_text(&source(&format!("spartan://127.0.0.1:{port}/")))
            .expect("spartan redirect fetch should succeed");

        assert_eq!(
            fetch.content_kind_override,
            Some(MiddleNetContentKind::GeminiText)
        );
        assert_eq!(fetch.body, "=> /done Done\n");
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn compose_titan_request_target_inserts_parameters_before_query() {
        let url = url::Url::parse("gemini://example.org/raw/Test?username=Alex")
            .expect("url should parse");

        let target = compose_titan_request_target(&url, 10, Some("text/plain"), Some("hello"))
            .expect("Titan target should compose");

        assert_eq!(
            target,
            "titan://example.org/raw/Test;size=10;mime=text/plain;token=hello?username=Alex"
        );
    }

    #[test]
    fn content_type_mapping_recognizes_json_feed() {
        assert_eq!(
            content_kind_from_content_type("application/feed+json; charset=utf-8"),
            Some(MiddleNetContentKind::JsonFeed)
        );
    }

    #[test]
    fn titan_upload_sends_request_and_receives_gemini_response() {
        let tempdir = TempDir::new().expect("temp dir should be created");
        let known_hosts =
            GeminiKnownHostsStore::new_for_tests(tempdir.path().join("gemini_known_hosts.json"));
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let config = build_test_tls_config("localhost");
            let (stream, _) = listener.accept().expect("accept");
            let mut tls = StreamOwned::new(
                ServerConnection::new(Arc::new(config)).expect("server connection"),
                stream,
            );
            let mut reader = BufReader::new(tls);
            let mut request = String::new();
            reader.read_line(&mut request).expect("request line");
            assert_eq!(
                request,
                format!(
                    "titan://localhost:{port}/raw/upload;size=11;mime=text/plain;token=secret\r\n"
                )
            );

            let mut body = [0_u8; 11];
            reader.read_exact(&mut body).expect("content body");
            assert_eq!(&body, b"hello titan");

            tls = reader.into_inner();
            tls.write_all(b"20 text/gemini\r\nUpload succeeded\n")
                .expect("response");
            tls.flush().expect("flush");
        });

        let url = url::Url::parse(&format!("gemini://localhost:{port}/raw/upload"))
            .expect("url should parse");
        let outcome = titan_upload_with_store(
            &url,
            b"hello titan",
            Some("text/plain"),
            Some("secret"),
            &known_hosts,
        )
        .expect("Titan upload should succeed");

        assert_eq!(outcome.status, 20);
        assert_eq!(outcome.meta, "text/gemini");
        assert_eq!(outcome.body, "Upload succeeded\n");
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn fetch_gemini_text_reads_success_body_and_records_known_host() {
        let tempdir = TempDir::new().expect("temp dir should be created");
        let known_hosts =
            GeminiKnownHostsStore::new_for_tests(tempdir.path().join("gemini_known_hosts.json"));
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let config = build_test_tls_config("localhost");
            let (stream, _) = listener.accept().expect("accept");
            let mut tls = StreamOwned::new(
                ServerConnection::new(Arc::new(config)).expect("server connection"),
                stream,
            );
            let mut reader = BufReader::new(tls);
            let mut request = String::new();
            reader.read_line(&mut request).expect("request line");
            assert!(request.starts_with("gemini://localhost:"));
            tls = reader.into_inner();
            tls.write_all(b"20 text/gemini\r\n# Hello\n")
                .expect("response");
            tls.flush().expect("flush");
        });

        let url =
            url::Url::parse(&format!("gemini://localhost:{port}/start")).expect("url should parse");
        let fetch =
            fetch_gemini_text_with_store(&url, &known_hosts).expect("gemini fetch should succeed");

        assert_eq!(
            fetch.content_kind_override,
            Some(MiddleNetContentKind::GeminiText)
        );
        assert_eq!(fetch.body, "# Hello\n");
        assert!(
            fs::read_to_string(tempdir.path().join("gemini_known_hosts.json"))
                .expect("known-hosts file should exist")
                .contains("localhost:")
        );
        server.join().expect("server joins cleanly");
    }

    #[test]
    fn known_hosts_reject_changed_certificate() {
        let tempdir = TempDir::new().expect("temp dir should be created");
        let known_hosts =
            GeminiKnownHostsStore::new_for_tests(tempdir.path().join("gemini_known_hosts.json"));
        let first = CertificateDer::from(vec![1_u8, 2, 3]);
        let second = CertificateDer::from(vec![4_u8, 5, 6]);

        known_hosts
            .remember_or_verify("example.com:1965", &first)
            .expect("first certificate should be accepted");
        let error = known_hosts
            .remember_or_verify("example.com:1965", &second)
            .expect_err("changed certificate should be rejected");

        assert!(error.contains("certificate changed"));
    }

    #[test]
    fn known_hosts_corruption_resets_to_empty() {
        let tempdir = TempDir::new().expect("temp dir should be created");
        let path = tempdir.path().join("gemini_known_hosts.json");
        fs::write(&path, "{ not valid json").expect("corrupt known-hosts should write");

        let records = load_known_hosts_from_path(&path)
            .expect("corrupt known-hosts should recover to an empty set");

        assert!(records.is_empty());
        assert_eq!(
            fs::read_to_string(&path).expect("recovered known-hosts should be readable"),
            "[]"
        );
    }

    fn build_test_tls_config(hostname: &str) -> ServerConfig {
        let key_pair = KeyPair::generate().expect("keypair should generate");
        let mut params = CertificateParams::new(vec![hostname.to_string()])
            .expect("certificate params should build");
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(2099, 12, 31);

        let cert = params
            .self_signed(&key_pair)
            .expect("self-signed cert should build");
        let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
        let key_der = rustls::pki_types::PrivateKeyDer::try_from(key_pair.serialize_der())
            .expect("key der should convert");

        ServerConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
            .with_protocol_versions(rustls::DEFAULT_VERSIONS)
            .expect("rustls default protocol versions should be valid for Gemini test server")
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("server config should build")
    }
}
