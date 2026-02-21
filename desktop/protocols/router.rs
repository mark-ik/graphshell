/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Thin registration router for custom protocol handlers.
//!
//! This keeps protocol wiring centralized so new schemes can be plugged in
//! without touching app initialization logic.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use reqwest::blocking::Client;
use servo::protocol_handler::{ProtocolHandler, ProtocolRegistry};
use url::Url;

use crate::desktop::protocols;

pub(crate) struct AppSchemeRouter {
    registry: ProtocolRegistry,
}

impl AppSchemeRouter {
    pub(crate) fn new() -> Self {
        Self {
            registry: ProtocolRegistry::default(),
        }
    }

    pub(crate) fn register<H>(&mut self, scheme: &'static str, handler: H) -> bool
    where
        H: ProtocolHandler + 'static,
    {
        self.registry.register(scheme, handler).is_ok()
    }

    pub(crate) fn register_default_handlers(&mut self) {
        let _ = self.register(
            "urlinfo",
            protocols::urlinfo::UrlInfoProtocolHander::default(),
        );
        let _ = self.register("servo", protocols::servo::ServoProtocolHandler::default());
        let _ = self.register(
            "resource",
            protocols::resource::ResourceProtocolHandler::default(),
        );
    }

    pub(crate) fn into_registry(self) -> ProtocolRegistry {
        self.registry
    }
}

impl Default for AppSchemeRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub(crate) enum OutboundFetchError {
    InvalidUrl,
    UnsupportedScheme,
    Network,
    HttpStatus(u16),
    Body,
}

pub(crate) trait OutboundSchemeHandler: Send + Sync {
    fn fetch_text(&self, url: &Url) -> Result<String, OutboundFetchError>;
}

impl<F> OutboundSchemeHandler for F
where
    F: Fn(&Url) -> Result<String, OutboundFetchError> + Send + Sync,
{
    fn fetch_text(&self, url: &Url) -> Result<String, OutboundFetchError> {
        self(url)
    }
}

struct OutboundSchemeRouter {
    handlers: HashMap<String, Box<dyn OutboundSchemeHandler>>,
}

impl OutboundSchemeRouter {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    fn register<H>(&mut self, scheme: &str, handler: H)
    where
        H: OutboundSchemeHandler + 'static,
    {
        self.handlers
            .insert(scheme.to_ascii_lowercase(), Box::new(handler));
    }

    fn fetch_text(&self, parsed: &Url) -> Result<String, OutboundFetchError> {
        let Some(handler) = self.handlers.get(parsed.scheme()) else {
            return Err(OutboundFetchError::UnsupportedScheme);
        };
        handler.fetch_text(parsed)
    }
}

fn default_outbound_router() -> OutboundSchemeRouter {
    let mut router = OutboundSchemeRouter::new();
    router.register("http", fetch_http_text);
    router.register("https", fetch_http_text);
    router
}

fn outbound_router() -> &'static RwLock<OutboundSchemeRouter> {
    static ROUTER: OnceLock<RwLock<OutboundSchemeRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| RwLock::new(default_outbound_router()))
}

pub(crate) fn register_outbound_scheme_handler<H>(scheme: &str, handler: H)
where
    H: OutboundSchemeHandler + 'static,
{
    if let Ok(mut router) = outbound_router().write() {
        router.register(scheme, handler);
    }
}

fn outbound_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .build()
            .expect("reqwest blocking client should build")
    })
}

fn fetch_http_text(url: &Url) -> Result<String, OutboundFetchError> {
    let response = outbound_client()
        .get(url.clone())
        .send()
        .map_err(|_| OutboundFetchError::Network)?;
    let status = response.status();
    if !status.is_success() {
        return Err(OutboundFetchError::HttpStatus(status.as_u16()));
    }
    response.text().map_err(|_| OutboundFetchError::Body)
}

pub(crate) fn fetch_text(url: &str) -> Result<String, OutboundFetchError> {
    let parsed = Url::parse(url).map_err(|_| OutboundFetchError::InvalidUrl)?;
    if let Ok(router) = outbound_router().read() {
        return router.fetch_text(&parsed);
    }
    Err(OutboundFetchError::Network)
}
