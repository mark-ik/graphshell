/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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

#[cfg(test)]
mod tests {
    use super::*;

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
}