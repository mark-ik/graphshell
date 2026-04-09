/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use nostr::ToBech32;
use reqwest::header::ACCEPT;
use serde::Deserialize;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const IDENTITY_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);
const ACTIVITYPUB_ACCEPT: &str = "application/activity+json, application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\", application/ld+json;q=0.9, application/json;q=0.8";

#[cfg(test)]
#[derive(Clone)]
struct TestResolveProfileOverride {
    resource: String,
    result: Result<PersonIdentityProfile, String>,
}

#[cfg(test)]
fn test_resolve_override_run_lock() -> &'static Mutex<()> {
    static RUN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    RUN_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn test_nip05_override() -> &'static Mutex<Option<TestResolveProfileOverride>> {
    static OVERRIDE: OnceLock<Mutex<Option<TestResolveProfileOverride>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn test_matrix_override() -> &'static Mutex<Option<TestResolveProfileOverride>> {
    static OVERRIDE: OnceLock<Mutex<Option<TestResolveProfileOverride>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn test_activitypub_override() -> &'static Mutex<Option<TestResolveProfileOverride>> {
    static OVERRIDE: OnceLock<Mutex<Option<TestResolveProfileOverride>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PersonIdentityProfile {
    pub(crate) human_handle: Option<String>,
    pub(crate) webfinger_resource: Option<String>,
    pub(crate) nip05_identifier: Option<String>,
    pub(crate) matrix_mxids: Vec<String>,
    pub(crate) nostr_identities: Vec<String>,
    pub(crate) misfin_mailboxes: Vec<String>,
    pub(crate) gemini_capsules: Vec<String>,
    pub(crate) gopher_resources: Vec<String>,
    pub(crate) activitypub_actors: Vec<String>,
    pub(crate) profile_pages: Vec<String>,
    pub(crate) aliases: Vec<String>,
    pub(crate) other_endpoints: Vec<crate::middlenet::webfinger::WebFingerEndpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersonArtifactKind {
    Post,
    SharedData,
    MessageNotification,
}

impl PersonArtifactKind {
    pub(crate) fn route_segment(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::SharedData => "shared-data",
            Self::MessageNotification => "message-notification",
        }
    }

    pub(crate) fn title_prefix(self) -> &'static str {
        match self {
            Self::Post => "Post",
            Self::SharedData => "Shared Data",
            Self::MessageNotification => "Message Notification",
        }
    }

    pub(crate) fn relation_label(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::SharedData => "shared-data",
            Self::MessageNotification => "message-notification",
        }
    }
}

impl PersonIdentityProfile {
    pub(crate) fn merge(&mut self, other: &Self) {
        if self.human_handle.is_none() {
            self.human_handle = other.human_handle.clone();
        }
        if self.webfinger_resource.is_none() {
            self.webfinger_resource = other.webfinger_resource.clone();
        }
        if self.nip05_identifier.is_none() {
            self.nip05_identifier = other.nip05_identifier.clone();
        }

        merge_unique(&mut self.matrix_mxids, &other.matrix_mxids);
        merge_unique(&mut self.nostr_identities, &other.nostr_identities);
        merge_unique(&mut self.misfin_mailboxes, &other.misfin_mailboxes);
        merge_unique(&mut self.gemini_capsules, &other.gemini_capsules);
        merge_unique(&mut self.gopher_resources, &other.gopher_resources);
        merge_unique(&mut self.activitypub_actors, &other.activitypub_actors);
        merge_unique(&mut self.profile_pages, &other.profile_pages);
        merge_unique(&mut self.aliases, &other.aliases);
        for endpoint in &other.other_endpoints {
            if self
                .other_endpoints
                .iter()
                .any(|existing| existing.rel == endpoint.rel && existing.href == endpoint.href)
            {
                continue;
            }
            self.other_endpoints.push(endpoint.clone());
        }
    }

    pub(crate) fn from_webfinger_import(
        resource: &str,
        import: &crate::middlenet::webfinger::WebFingerImport,
    ) -> Result<Self, String> {
        let normalized_resource = crate::middlenet::webfinger::normalize_resource(resource)?;
        let subject = crate::middlenet::webfinger::normalize_resource(&import.subject)
            .unwrap_or_else(|_| normalized_resource.clone());
        let mut profile = Self {
            human_handle: subject.strip_prefix("acct:").map(str::to_string),
            webfinger_resource: Some(subject.clone()),
            ..Self::default()
        };

        if normalized_resource != subject {
            profile.push_alias(normalized_resource);
        }

        for alias in &import.aliases {
            profile.push_alias(alias.clone());
        }
        for page in &import.profile_pages {
            profile.push_profile_page(page)?;
        }
        for capsule in &import.gemini_capsules {
            profile.push_gemini_capsule(capsule)?;
        }
        for resource in &import.gopher_resources {
            profile.push_gopher_resource(resource)?;
        }
        for mailbox in &import.misfin_mailboxes {
            profile.push_misfin_mailbox(mailbox)?;
        }
        for identity in &import.nostr_identities {
            profile.push_nostr_identity(identity)?;
        }
        for actor in &import.activitypub_actors {
            profile.push_activitypub_actor(actor)?;
        }
        for endpoint in &import.other_endpoints {
            if profile
                .other_endpoints
                .iter()
                .any(|existing| existing.href == endpoint.href && existing.rel == endpoint.rel)
            {
                continue;
            }
            profile.other_endpoints.push(endpoint.clone());
        }

        Ok(profile)
    }

    pub(crate) fn preferred_label(&self) -> &str {
        self.human_handle
            .as_deref()
            .or(self.nip05_identifier.as_deref())
            .or(self.webfinger_resource.as_deref())
            .or_else(|| self.matrix_mxids.first().map(String::as_str))
            .or_else(|| self.nostr_identities.first().map(String::as_str))
            .or_else(|| self.misfin_mailboxes.first().map(String::as_str))
            .or_else(|| self.activitypub_actors.first().map(String::as_str))
            .or_else(|| self.gemini_capsules.first().map(String::as_str))
            .or_else(|| self.profile_pages.first().map(String::as_str))
            .or_else(|| self.aliases.first().map(String::as_str))
            .unwrap_or("person")
    }

    pub(crate) fn canonical_identity(&self) -> Option<&str> {
        self.webfinger_resource
            .as_deref()
            .or(self.nip05_identifier.as_deref())
            .or_else(|| self.matrix_mxids.first().map(String::as_str))
            .or_else(|| self.nostr_identities.first().map(String::as_str))
            .or_else(|| self.misfin_mailboxes.first().map(String::as_str))
            .or_else(|| self.activitypub_actors.first().map(String::as_str))
            .or_else(|| self.gemini_capsules.first().map(String::as_str))
            .or_else(|| self.profile_pages.first().map(String::as_str))
            .or_else(|| self.aliases.first().map(String::as_str))
    }

    pub(crate) fn canonical_seed(&self) -> Option<String> {
        self.human_handle
            .as_deref()
            .or(self.nip05_identifier.as_deref())
            .or_else(|| {
                self.webfinger_resource
                    .as_deref()
                    .and_then(|resource| resource.strip_prefix("acct:"))
            })
            .map(|handle| format!("handle:{}", handle.to_ascii_lowercase()))
            .or_else(|| self.canonical_identity().map(str::to_string))
    }

    pub(crate) fn set_nip05_identifier(&mut self, input: &str) -> Result<(), String> {
        self.nip05_identifier = Some(normalize_nip05_identifier(input)?);
        Ok(())
    }

    pub(crate) fn push_matrix_mxid(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.matrix_mxids,
            normalize_matrix_mxid(input)?,
        );
        Ok(())
    }

    pub(crate) fn push_nostr_identity(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.nostr_identities,
            normalize_nostr_identity(input)?,
        );
        Ok(())
    }

    pub(crate) fn push_misfin_mailbox(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.misfin_mailboxes,
            normalize_misfin_mailbox(input)?,
        );
        Ok(())
    }

    pub(crate) fn push_gemini_capsule(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.gemini_capsules,
            normalize_url_with_scheme(input, "gemini", "Gemini capsule")?,
        );
        Ok(())
    }

    pub(crate) fn push_gopher_resource(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.gopher_resources,
            normalize_url_with_scheme(input, "gopher", "Gopher resource")?,
        );
        Ok(())
    }

    pub(crate) fn push_activitypub_actor(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.activitypub_actors,
            normalize_httpish_url(input, "ActivityPub actor")?,
        );
        Ok(())
    }

    pub(crate) fn push_profile_page(&mut self, input: &str) -> Result<(), String> {
        push_unique(
            &mut self.profile_pages,
            normalize_httpish_url(input, "profile page")?,
        );
        Ok(())
    }

    pub(crate) fn push_alias(&mut self, input: String) {
        push_unique(&mut self.aliases, input);
    }
}

pub(crate) fn normalize_nip05_identifier(input: &str) -> Result<String, String> {
    normalize_account_like(input.trim_start_matches("nip05:"), "NIP-05 identifier")
}

pub(crate) fn normalize_matrix_mxid(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let mxid = trimmed
        .strip_prefix('@')
        .ok_or_else(|| format!("Matrix MXID '{trimmed}' must start with '@'."))?;
    let (localpart, server) = mxid.split_once(':').ok_or_else(|| {
        format!("Matrix MXID '{trimmed}' must include a ':server' suffix.")
    })?;
    if localpart.trim().is_empty() || server.trim().is_empty() {
        return Err(format!("Matrix MXID '{trimmed}' is incomplete."));
    }
    Ok(format!("@{}:{}", localpart.trim(), server.trim().to_ascii_lowercase()))
}

pub(crate) fn normalize_nostr_identity(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let identity = trimmed.strip_prefix("nostr:").unwrap_or(trimmed).trim();
    if identity.is_empty() {
        return Err("Nostr identity cannot be empty.".to_string());
    }
    if !(identity.starts_with("npub1") || identity.starts_with("nprofile1")) {
        return Err(format!(
            "Unsupported Nostr identity '{trimmed}'. Expected an npub or nprofile identifier."
        ));
    }
    Ok(format!("nostr:{identity}"))
}

pub(crate) fn normalize_misfin_mailbox(input: &str) -> Result<String, String> {
    let url = url::Url::parse(input.trim())
        .map_err(|error| format!("Invalid Misfin mailbox '{}': {error}", input.trim()))?;
    if url.scheme() != "misfin" {
        return Err(format!(
            "Invalid Misfin mailbox '{}': expected misfin:// scheme.",
            input.trim()
        ));
    }
    let address = crate::middlenet::misfin::MisfinAddress::from_url(&url)?;
    if let Some(port) = url.port() {
        Ok(format!("misfin://{}:{}", address.as_addr_spec(), port))
    } else {
        Ok(format!("misfin://{}", address.as_addr_spec()))
    }
}

fn normalize_account_like(input: &str, label: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let (localpart, host) = trimmed.split_once('@').ok_or_else(|| {
        format!("{label} '{trimmed}' must contain a local part and host.")
    })?;
    if localpart.trim().is_empty() || host.trim().is_empty() {
        return Err(format!("{label} '{trimmed}' is incomplete."));
    }
    Ok(format!("{}@{}", localpart.trim(), host.trim().to_ascii_lowercase()))
}

fn normalize_httpish_url(input: &str, label: &str) -> Result<String, String> {
    let url = url::Url::parse(input.trim())
        .map_err(|error| format!("Invalid {label} '{}': {error}", input.trim()))?;
    match url.scheme() {
        "http" | "https" => Ok(url.to_string()),
        _ => Err(format!(
            "Invalid {label} '{}': expected http:// or https://.",
            input.trim()
        )),
    }
}

fn normalize_url_with_scheme(input: &str, expected_scheme: &str, label: &str) -> Result<String, String> {
    let url = url::Url::parse(input.trim())
        .map_err(|error| format!("Invalid {label} '{}': {error}", input.trim()))?;
    if url.scheme() != expected_scheme {
        return Err(format!(
            "Invalid {label} '{}': expected {}:// scheme.",
            input.trim(),
            expected_scheme
        ));
    }
    Ok(url.to_string())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn merge_unique(values: &mut Vec<String>, additions: &[String]) {
    for value in additions {
        push_unique(values, value.clone());
    }
}

#[derive(Debug, Deserialize)]
struct Nip05Document {
    #[serde(default)]
    names: BTreeMap<String, String>,
    #[serde(default)]
    relays: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MatrixDiscoveryDocument {
    #[serde(rename = "m.homeserver")]
    homeserver: Option<MatrixHomeserverDocument>,
}

#[derive(Debug, Deserialize)]
struct MatrixHomeserverDocument {
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct MatrixProfileDocument {
    #[serde(default, rename = "displayname")]
    display_name: Option<String>,
    #[serde(default, rename = "avatar_url")]
    avatar_url: Option<String>,
}

pub(crate) fn resolve_nip05_profile(identifier: &str) -> Result<PersonIdentityProfile, String> {
    #[cfg(test)]
    {
        if let Some(override_state) = test_nip05_override()
            .lock()
            .expect("nip05 test resolve override lock poisoned")
            .as_ref()
            .filter(|override_state| override_state.resource == identifier)
            .cloned()
        {
            return override_state.result;
        }
    }

    let normalized = normalize_nip05_identifier(identifier)?;
    let (_, host) = normalized
        .split_once('@')
        .ok_or_else(|| format!("NIP-05 identifier '{normalized}' is incomplete."))?;
    let origin = url::Url::parse(&format!("https://{host}/"))
        .map_err(|error| format!("Invalid NIP-05 origin for '{normalized}': {error}"))?;
    resolve_nip05_profile_with_origin(&normalized, &origin)
}

pub(crate) fn resolve_matrix_profile(mxid: &str) -> Result<PersonIdentityProfile, String> {
    #[cfg(test)]
    {
        if let Some(override_state) = test_matrix_override()
            .lock()
            .expect("matrix test resolve override lock poisoned")
            .as_ref()
            .filter(|override_state| override_state.resource == mxid)
            .cloned()
        {
            return override_state.result;
        }
    }

    let normalized = normalize_matrix_mxid(mxid)?;
    let (_, server) = normalized
        .split_once(':')
        .ok_or_else(|| format!("Matrix MXID '{normalized}' is incomplete."))?;
    let origin = url::Url::parse(&format!("https://{server}/"))
        .map_err(|error| format!("Invalid Matrix discovery origin for '{normalized}': {error}"))?;
    resolve_matrix_profile_with_origin(&normalized, &origin)
}

pub(crate) fn resolve_activitypub_actor(actor_url: &str) -> Result<PersonIdentityProfile, String> {
    #[cfg(test)]
    {
        if let Some(override_state) = test_activitypub_override()
            .lock()
            .expect("activitypub test resolve override lock poisoned")
            .as_ref()
            .filter(|override_state| override_state.resource == actor_url)
            .cloned()
        {
            return override_state.result;
        }
    }

    let actor_url = normalize_httpish_url(actor_url, "ActivityPub actor")?;
    let client = identity_http_client()?;
    let actor_endpoint = url::Url::parse(&actor_url)
        .map_err(|error| format!("Invalid ActivityPub actor URL '{actor_url}': {error}"))?;
    let body = fetch_text(&client, &actor_endpoint, Some(ACTIVITYPUB_ACCEPT), "ActivityPub actor")?;
    let document: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("ActivityPub actor parse failed for '{actor_url}': {error}"))?;

    let mut profile = PersonIdentityProfile::default();
    profile.push_activitypub_actor(&actor_url)?;

    if let Some(actor_id) = document
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != actor_url)
    {
        profile.push_activitypub_actor(actor_id)?;
    }

    let actor_host = document
        .get("id")
        .and_then(serde_json::Value::as_str)
        .or(Some(actor_url.as_str()))
        .and_then(|value| url::Url::parse(value).ok())
        .and_then(|url| url.host_str().map(str::to_string));

    if let Some(preferred_username) = document
        .get("preferredUsername")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && profile.human_handle.is_none()
        && let Some(host) = actor_host
    {
        profile.human_handle = Some(format!("{}@{}", preferred_username, host.to_ascii_lowercase()));
    }

    for page in extract_activitypub_urls(document.get("url")) {
        let _ = profile.push_profile_page(&page);
    }
    for alias in extract_activitypub_urls(document.get("alsoKnownAs")) {
        profile.push_alias(alias);
    }
    for (rel, field_name) in [
        ("inbox", "inbox"),
        ("outbox", "outbox"),
        ("followers", "followers"),
        ("following", "following"),
        ("featured", "featured"),
    ] {
        if let Some(href) = document
            .get(field_name)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            profile.other_endpoints.push(crate::middlenet::webfinger::WebFingerEndpoint {
                rel: rel.to_string(),
                media_type: Some("application/activity+json".to_string()),
                href: href.to_string(),
            });
        }
    }

    if let Some(shared_inbox) = document
        .get("endpoints")
        .and_then(|value| value.get("sharedInbox"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile.other_endpoints.push(crate::middlenet::webfinger::WebFingerEndpoint {
            rel: "shared-inbox".to_string(),
            media_type: Some("application/activity+json".to_string()),
            href: shared_inbox.to_string(),
        });
    }

    for icon_url in extract_activitypub_urls(document.get("icon")) {
        profile.other_endpoints.push(crate::middlenet::webfinger::WebFingerEndpoint {
            rel: "icon".to_string(),
            media_type: None,
            href: icon_url,
        });
    }

    Ok(profile)
}

fn resolve_nip05_profile_with_origin(
    identifier: &str,
    origin: &url::Url,
) -> Result<PersonIdentityProfile, String> {
    let normalized = normalize_nip05_identifier(identifier)?;
    let (localpart, _) = normalized
        .split_once('@')
        .ok_or_else(|| format!("NIP-05 identifier '{normalized}' is incomplete."))?;
    let endpoint = nip05_endpoint(origin, localpart)?;
    let client = identity_http_client()?;
    let body = fetch_text(&client, &endpoint, Some("application/json"), "NIP-05 document")?;
    let document: Nip05Document = serde_json::from_str(&body)
        .map_err(|error| format!("NIP-05 document parse failed for '{normalized}': {error}"))?;
    let pubkey = document.names.get(localpart).ok_or_else(|| {
        format!("NIP-05 document for '{normalized}' did not contain a pubkey for '{localpart}'.")
    })?;
    let npub = nostr::PublicKey::parse(pubkey.trim())
        .map_err(|error| format!("NIP-05 pubkey decode failed for '{normalized}': {error}"))?
        .to_bech32()
        .map_err(|error| format!("NIP-05 pubkey bech32 conversion failed for '{normalized}': {error}"))?;

    let mut profile = PersonIdentityProfile {
        human_handle: Some(normalized.clone()),
        nip05_identifier: Some(normalized.clone()),
        ..Default::default()
    };
    profile.push_nostr_identity(&npub)?;
    if let Some(relays) = document.relays.get(pubkey) {
        for relay in relays {
            let relay = relay.trim();
            if relay.is_empty() {
                continue;
            }
            profile.other_endpoints.push(crate::middlenet::webfinger::WebFingerEndpoint {
                rel: "nostr-relay".to_string(),
                media_type: None,
                href: relay.to_string(),
            });
        }
    }
    Ok(profile)
}

fn resolve_matrix_profile_with_origin(
    mxid: &str,
    discovery_origin: &url::Url,
) -> Result<PersonIdentityProfile, String> {
    let normalized = normalize_matrix_mxid(mxid)?;
    let homeserver_base_url = resolve_matrix_homeserver_base_url(discovery_origin)?;
    let endpoint = matrix_profile_endpoint(&homeserver_base_url, &normalized)?;
    let client = identity_http_client()?;
    let body = fetch_text(&client, &endpoint, Some("application/json"), "Matrix profile")?;
    let document: MatrixProfileDocument = serde_json::from_str(&body)
        .map_err(|error| format!("Matrix profile parse failed for '{normalized}': {error}"))?;

    let mut profile = PersonIdentityProfile::default();
    profile.push_matrix_mxid(&normalized)?;
    let _ = profile.push_profile_page(&matrix_to_profile_url(&normalized));
    if let Some(display_name) = document.display_name.as_deref().map(str::trim)
        && !display_name.is_empty()
        && profile.human_handle.is_none()
    {
        profile.human_handle = Some(display_name.to_string());
    }
    if let Some(avatar_url) = document.avatar_url.as_deref().map(str::trim)
        && !avatar_url.is_empty()
        && let Some(download_url) = matrix_avatar_download_url(&homeserver_base_url, avatar_url)
    {
        profile.other_endpoints.push(crate::middlenet::webfinger::WebFingerEndpoint {
            rel: "matrix-avatar".to_string(),
            media_type: None,
            href: download_url,
        });
    }
    Ok(profile)
}

fn identity_http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(IDENTITY_RESOLUTION_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to build identity HTTP client: {error}"))
}

fn fetch_text(
    client: &reqwest::blocking::Client,
    url: &url::Url,
    accept: Option<&str>,
    label: &str,
) -> Result<String, String> {
    let mut request = client.get(url.as_str());
    if let Some(accept) = accept {
        request = request.header(ACCEPT, accept);
    }
    request
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("{label} request failed for '{url}': {error}"))?
        .text()
        .map_err(|error| format!("{label} response decode failed for '{url}': {error}"))
}

fn nip05_endpoint(origin: &url::Url, localpart: &str) -> Result<url::Url, String> {
    let mut endpoint = origin
        .join("/.well-known/nostr.json")
        .map_err(|error| format!("Failed to build NIP-05 endpoint URL: {error}"))?;
    endpoint.set_query(None);
    endpoint.query_pairs_mut().append_pair("name", localpart);
    Ok(endpoint)
}

fn resolve_matrix_homeserver_base_url(discovery_origin: &url::Url) -> Result<url::Url, String> {
    let client = identity_http_client()?;
    let discovery_url = discovery_origin
        .join("/.well-known/matrix/client")
        .map_err(|error| format!("Failed to build Matrix discovery URL: {error}"))?;
    let response = client
        .get(discovery_url.as_str())
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| format!("Matrix discovery request failed for '{discovery_url}': {error}"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(discovery_origin.clone());
    }

    let response = response
        .error_for_status()
        .map_err(|error| format!("Matrix discovery request failed for '{discovery_url}': {error}"))?;
    let body = response
        .text()
        .map_err(|error| format!("Matrix discovery response decode failed for '{discovery_url}': {error}"))?;
    let document: MatrixDiscoveryDocument = serde_json::from_str(&body)
        .map_err(|error| format!("Matrix discovery parse failed for '{discovery_url}': {error}"))?;

    document
        .homeserver
        .and_then(|homeserver| url::Url::parse(homeserver.base_url.trim()).ok())
        .or_else(|| Some(discovery_origin.clone()))
        .ok_or_else(|| format!("Matrix discovery base URL was invalid for '{discovery_url}'."))
}

fn matrix_profile_endpoint(base_url: &url::Url, mxid: &str) -> Result<url::Url, String> {
    let encoded_mxid = url::form_urlencoded::byte_serialize(mxid.as_bytes()).collect::<String>();
    base_url
        .join(&format!("/_matrix/client/v3/profile/{encoded_mxid}"))
        .map_err(|error| format!("Failed to build Matrix profile URL: {error}"))
}

fn matrix_avatar_download_url(base_url: &url::Url, avatar_url: &str) -> Option<String> {
    if let Ok(parsed) = url::Url::parse(avatar_url) {
        match parsed.scheme() {
            "http" | "https" => return Some(parsed.to_string()),
            "mxc" => {
                let host = parsed.host_str()?;
                let media_id = parsed.path().trim_start_matches('/');
                if media_id.is_empty() {
                    return None;
                }
                return base_url
                    .join(&format!("/_matrix/media/v3/download/{host}/{media_id}"))
                    .ok()
                    .map(|url| url.to_string());
            }
            _ => return None,
        }
    }
    None
}

fn matrix_to_profile_url(mxid: &str) -> String {
    format!("https://matrix.to/#/{mxid}")
}

fn extract_activitypub_urls(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    match value {
        serde_json::Value::String(text) => vec![text.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|item| extract_activitypub_urls(Some(item)))
            .collect(),
        serde_json::Value::Object(map) => ["href", "url", "id"]
            .iter()
            .filter_map(|key| map.get(*key))
            .flat_map(|item| extract_activitypub_urls(Some(item)))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
pub(crate) fn with_test_resolve_nip05_override<T>(
    resource: &str,
    result: Result<PersonIdentityProfile, String>,
    run: impl FnOnce() -> T,
) -> T {
    let _run_lock = test_resolve_override_run_lock()
        .lock()
        .expect("nip05 test resolve override lock poisoned");
    let previous = {
        let mut override_slot = test_nip05_override()
            .lock()
            .expect("nip05 test resolve override lock poisoned");
        override_slot.replace(TestResolveProfileOverride {
            resource: resource.to_string(),
            result,
        })
    };
    let outcome = run();
    *test_nip05_override()
        .lock()
        .expect("nip05 test resolve override lock poisoned") = previous;
    outcome
}

#[cfg(test)]
pub(crate) fn with_test_resolve_matrix_override<T>(
    resource: &str,
    result: Result<PersonIdentityProfile, String>,
    run: impl FnOnce() -> T,
) -> T {
    let _run_lock = test_resolve_override_run_lock()
        .lock()
        .expect("matrix test resolve override lock poisoned");
    let previous = {
        let mut override_slot = test_matrix_override()
            .lock()
            .expect("matrix test resolve override lock poisoned");
        override_slot.replace(TestResolveProfileOverride {
            resource: resource.to_string(),
            result,
        })
    };
    let outcome = run();
    *test_matrix_override()
        .lock()
        .expect("matrix test resolve override lock poisoned") = previous;
    outcome
}

#[cfg(test)]
pub(crate) fn with_test_resolve_activitypub_override<T>(
    resource: &str,
    result: Result<PersonIdentityProfile, String>,
    run: impl FnOnce() -> T,
) -> T {
    let _run_lock = test_resolve_override_run_lock()
        .lock()
        .expect("activitypub test resolve override lock poisoned");
    let previous = {
        let mut override_slot = test_activitypub_override()
            .lock()
            .expect("activitypub test resolve override lock poisoned");
        override_slot.replace(TestResolveProfileOverride {
            resource: resource.to_string(),
            result,
        })
    };
    let outcome = run();
    *test_activitypub_override()
        .lock()
        .expect("activitypub test resolve override lock poisoned") = previous;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn normalize_nip05_identifier_accepts_plain_identifier() {
        assert_eq!(
            normalize_nip05_identifier("mark@example.net").expect("nip-05 should normalize"),
            "mark@example.net"
        );
    }

    #[test]
    fn normalize_matrix_mxid_requires_at_and_server() {
        assert_eq!(
            normalize_matrix_mxid("@mark:matrix.example").expect("mxid should normalize"),
            "@mark:matrix.example"
        );
        assert!(normalize_matrix_mxid("mark:matrix.example").is_err());
    }

    #[test]
    fn person_identity_profile_from_webfinger_collects_endpoints() {
        let import = crate::middlenet::webfinger::WebFingerImport {
            subject: "acct:mark@example.net".to_string(),
            aliases: vec!["https://example.net/~mark".to_string()],
            profile_pages: vec!["https://example.net/profile".to_string()],
            gemini_capsules: vec!["gemini://example.net/~mark".to_string()],
            gopher_resources: vec!["gopher://example.net/1/users/mark".to_string()],
            misfin_mailboxes: vec!["misfin://mark@example.net".to_string()],
            nostr_identities: vec!["nostr:npub1example".to_string()],
            activitypub_actors: vec!["https://example.net/users/mark".to_string()],
            other_endpoints: Vec::new(),
        };

        let profile = PersonIdentityProfile::from_webfinger_import(
            "mark@example.net",
            &import,
        )
        .expect("webfinger identity profile should build");

        assert_eq!(profile.human_handle.as_deref(), Some("mark@example.net"));
        assert_eq!(
            profile.webfinger_resource.as_deref(),
            Some("acct:mark@example.net")
        );
        assert!(profile
            .aliases
            .iter()
            .any(|alias| alias == "https://example.net/~mark"));
        assert!(profile
            .nostr_identities
            .iter()
            .any(|value| value == "nostr:npub1example"));
        assert!(profile
            .misfin_mailboxes
            .iter()
            .any(|value| value == "misfin://mark@example.net"));
    }

    #[test]
    fn resolve_nip05_profile_collects_nostr_identity_and_relays() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let pubkey_hex = {
            let secret_key = secp256k1::SecretKey::from_byte_array([7u8; 32])
                .expect("secret key should build");
            let secp = secp256k1::Secp256k1::new();
            let keypair = secp256k1::Keypair::from_secret_key(&secp, &secret_key);
            let (pubkey, _) = secp256k1::XOnlyPublicKey::from_keypair(&keypair);
            pubkey.to_string()
        };
        let expected_pubkey = pubkey_hex.clone();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            reader.read_line(&mut request_line).expect("request line");
            assert_eq!(
                request_line,
                "GET /.well-known/nostr.json?name=mark HTTP/1.1\r\n"
            );

            loop {
                let mut header = String::new();
                reader.read_line(&mut header).expect("header line");
                if header == "\r\n" {
                    break;
                }
            }

            let body = format!(
                "{{\"names\":{{\"mark\":\"{}\"}},\"relays\":{{\"{}\":[\"wss://relay.example.net\"]}}}}",
                expected_pubkey, expected_pubkey
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let mut writer = stream;
            writer.write_all(response.as_bytes()).expect("response write");
            writer.flush().expect("response flush");
        });

        let origin = url::Url::parse(&format!("http://127.0.0.1:{port}/")).expect("origin");
        let profile = resolve_nip05_profile_with_origin("mark@example.net", &origin)
            .expect("nip-05 resolution should succeed");

        assert_eq!(profile.nip05_identifier.as_deref(), Some("mark@example.net"));
        assert!(profile
            .nostr_identities
            .iter()
            .any(|identity| identity.starts_with("nostr:npub1")));
        assert!(profile.other_endpoints.iter().any(|endpoint| {
            endpoint.rel == "nostr-relay" && endpoint.href == "wss://relay.example.net"
        }));

        server.join().expect("server should finish");
    }

    #[test]
    fn resolve_matrix_profile_discovers_homeserver_and_avatar() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept");
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                reader.read_line(&mut request_line).expect("request line");
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).expect("header line");
                    if header == "\r\n" {
                        break;
                    }
                }

                let (status_line, body) = if request_line
                    == "GET /.well-known/matrix/client HTTP/1.1\r\n"
                {
                    (
                        "HTTP/1.1 200 OK",
                        format!(
                            "{{\"m.homeserver\":{{\"base_url\":\"http://127.0.0.1:{port}\"}}}}"
                        ),
                    )
                } else {
                    assert_eq!(
                        request_line,
                        "GET /_matrix/client/v3/profile/%40mark%3Amatrix.example HTTP/1.1\r\n"
                    );
                    (
                        "HTTP/1.1 200 OK",
                        "{\"displayname\":\"Mark Example\",\"avatar_url\":\"mxc://media.example.net/avatar123\"}".to_string(),
                    )
                };

                let response = format!(
                    "{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status_line,
                    body.len(),
                    body
                );
                let mut writer = stream;
                writer.write_all(response.as_bytes()).expect("response write");
                writer.flush().expect("response flush");
            }
        });

        let origin = url::Url::parse(&format!("http://127.0.0.1:{port}/")).expect("origin");
        let profile = resolve_matrix_profile_with_origin("@mark:matrix.example", &origin)
            .expect("matrix profile resolution should succeed");

        assert!(profile
            .matrix_mxids
            .iter()
            .any(|mxid| mxid == "@mark:matrix.example"));
        assert!(profile
            .profile_pages
            .iter()
            .any(|page| page == "https://matrix.to/#/@mark:matrix.example"));
        assert!(profile.other_endpoints.iter().any(|endpoint| {
            endpoint.rel == "matrix-avatar"
                && endpoint.href
                    == format!("http://127.0.0.1:{port}/_matrix/media/v3/download/media.example.net/avatar123")
        }));

        server.join().expect("server should finish");
    }

    #[test]
    fn resolve_activitypub_actor_collects_profile_and_inbox_endpoints() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            reader.read_line(&mut request_line).expect("request line");
            assert_eq!(request_line, "GET /users/mark HTTP/1.1\r\n");

            let mut saw_accept = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).expect("header line");
                if header == "\r\n" {
                    break;
                }
                if header.to_ascii_lowercase().starts_with("accept:")
                    && header.contains("application/activity+json")
                {
                    saw_accept = true;
                }
            }
            assert!(saw_accept);

            let body = format!(
                "{{\"id\":\"http://127.0.0.1:{port}/users/mark\",\"preferredUsername\":\"mark\",\"url\":\"https://social.example/@mark\",\"alsoKnownAs\":[\"https://example.net/~mark\"],\"inbox\":\"http://127.0.0.1:{port}/users/mark/inbox\",\"outbox\":\"http://127.0.0.1:{port}/users/mark/outbox\",\"icon\":{{\"url\":\"https://social.example/media/avatar.png\"}}}}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/activity+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let mut writer = stream;
            writer.write_all(response.as_bytes()).expect("response write");
            writer.flush().expect("response flush");
        });

        let profile = resolve_activitypub_actor(&format!("http://127.0.0.1:{port}/users/mark"))
            .expect("activitypub actor resolution should succeed");

        assert!(profile
            .activitypub_actors
            .iter()
            .any(|actor| actor == &format!("http://127.0.0.1:{port}/users/mark")));
        assert_eq!(profile.human_handle.as_deref(), Some("mark@127.0.0.1"));
        assert!(profile
            .profile_pages
            .iter()
            .any(|page| page == "https://social.example/@mark"));
        assert!(profile
            .aliases
            .iter()
            .any(|alias| alias == "https://example.net/~mark"));
        assert!(profile.other_endpoints.iter().any(|endpoint| {
            endpoint.rel == "inbox"
                && endpoint.href == format!("http://127.0.0.1:{port}/users/mark/inbox")
        }));
        assert!(profile.other_endpoints.iter().any(|endpoint| {
            endpoint.rel == "icon" && endpoint.href == "https://social.example/media/avatar.png"
        }));

        server.join().expect("server should finish");
    }
}