/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Re-export shim: `portable_now()` now lives in
//! `graphshell-runtime::portable_time` (relocated 2026-04-25 as part
//! of the canonical graphshell-runtime crate extraction lane). This
//! shim preserves the existing `shell::desktop::ui::portable_time::*`
//! import paths while existing call sites are migrated to consume the
//! runtime-crate path directly.

pub(crate) use graphshell_runtime::portable_time::portable_now;
