/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::path::PathBuf;

pub(crate) fn file_path_from_node_url(url: &str) -> Result<PathBuf, String> {
    let parsed = url::Url::parse(url).map_err(|err| format!("Invalid URL: {err}"))?;
    if parsed.scheme() != "file" {
        return Err("Embedded plaintext viewer currently supports file:// URLs only.".to_string());
    }

    parsed
        .to_file_path()
        .map_err(|_| "Could not convert file:// URL to local path.".to_string())
}

pub(crate) fn ensure_local_file_access_allowed(
    path: &PathBuf,
    policy: &crate::prefs::FileAccessPolicy,
) -> Result<(), String> {
    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve '{}': {err}", path.display()))?;

    for allowed in &policy.allowed_directories {
        if let Ok(canonical_allowed) = allowed.canonicalize()
            && canonical_path.starts_with(&canonical_allowed)
        {
            return Ok(());
        }
    }

    if policy.home_directory_auto_allow {
        if let Some(home_dir) = dirs::home_dir() {
            let canonical_home = home_dir.canonicalize().map_err(|err| {
                format!(
                    "Failed to resolve home directory '{}': {err}",
                    home_dir.display()
                )
            })?;
            if canonical_path.starts_with(&canonical_home) {
                return Ok(());
            }
        }
    }

    Err(format!(
        "Access denied for '{}'. Adjust file_access_policy in preferences to allow additional paths.",
        canonical_path.display()
    ))
}

pub(crate) fn guarded_file_path_from_node_url(
    url: &str,
    policy: &crate::prefs::FileAccessPolicy,
) -> Result<PathBuf, String> {
    let path = file_path_from_node_url(url)?;
    ensure_local_file_access_allowed(&path, policy)?;
    Ok(path)
}
