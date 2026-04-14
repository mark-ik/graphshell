/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const NIP07_PROMPT_PREFIX: &str = "graphshell:nip07:";

const BUILTIN_NIP07_BOOTSTRAP: &str = r#"(function () {
  if (typeof window === "undefined") {
    return;
  }
  if (Object.prototype.hasOwnProperty.call(window, "nostr")) {
    return;
  }

  const PREFIX = "graphshell:nip07:";

  function bridgeCall(method, params) {
    return new Promise((resolve, reject) => {
      let raw;
      try {
        raw = window.prompt(
          PREFIX + JSON.stringify({
            method,
            params,
            href: String(window.location && window.location.href || ""),
            origin: String(window.location && window.location.origin || "")
          }),
          ""
        );
      } catch (error) {
        reject(error);
        return;
      }

      if (typeof raw !== "string") {
        reject(new Error("Graphshell NIP-07 bridge unavailable"));
        return;
      }

      let response;
      try {
        response = JSON.parse(raw);
      } catch (_error) {
        reject(new Error("Graphshell NIP-07 bridge returned invalid JSON"));
        return;
      }

      if (!response || response.ok !== true) {
        reject(new Error((response && response.error) || "Graphshell NIP-07 request denied"));
        return;
      }

      resolve(response.result);
    });
  }

  const provider = {
    getPublicKey() {
      return bridgeCall("getPublicKey", null);
    },
    signEvent(event) {
      return bridgeCall("signEvent", event);
    },
    getRelays() {
      return bridgeCall("getRelays", null);
    }
  };

  Object.defineProperty(window, "nostr", {
    value: provider,
    configurable: false,
    enumerable: true,
    writable: false
  });
})();"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Nip07BridgeRequest {
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Value,
    #[serde(default)]
    pub(crate) href: Option<String>,
    #[serde(default)]
    pub(crate) origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Nip07BridgeResponse {
    pub(crate) ok: bool,
    #[serde(default)]
    pub(crate) result: Option<Value>,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

impl Nip07BridgeResponse {
    pub(crate) fn success(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

pub(crate) fn builtin_userscript_source() -> &'static str {
    BUILTIN_NIP07_BOOTSTRAP
}

pub(crate) fn try_parse_prompt_request(message: &str) -> Option<Nip07BridgeRequest> {
    let payload = message.strip_prefix(NIP07_PROMPT_PREFIX)?;
    serde_json::from_str(payload).ok()
}

pub(crate) fn try_handle_prompt_message<F>(message: &str, handler: F) -> Option<String>
where
    F: FnOnce(Nip07BridgeRequest) -> Nip07BridgeResponse,
{
    let request = try_parse_prompt_request(message)?;
    serde_json::to_string(&handler(request)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builtin_userscript_exposes_window_nostr() {
        let script = builtin_userscript_source();
        assert!(script.contains("window.prompt"));
        assert!(script.contains("getPublicKey"));
        assert!(script.contains("signEvent"));
        assert!(script.contains("getRelays"));
    }

    #[test]
    fn parse_prompt_request_requires_reserved_prefix() {
        assert!(try_parse_prompt_request("hello").is_none());
        let request =
            try_parse_prompt_request(r#"graphshell:nip07:{"method":"getPublicKey","params":null}"#)
                .expect("reserved prompt request should parse");
        assert_eq!(request.method, "getPublicKey");
    }

    #[test]
    fn handle_prompt_message_serializes_handler_response() {
        let encoded = try_handle_prompt_message(
            r#"graphshell:nip07:{"method":"signEvent","params":{"kind":1}}"#,
            |request| {
                assert_eq!(request.method, "signEvent");
                Nip07BridgeResponse::success(json!({"ok": "yes"}))
            },
        )
        .expect("reserved prompt should be handled");

        let response: Nip07BridgeResponse =
            serde_json::from_str(&encoded).expect("bridge response should be valid json");
        assert!(response.ok);
        assert_eq!(response.result, Some(json!({"ok": "yes"})));
    }
}

