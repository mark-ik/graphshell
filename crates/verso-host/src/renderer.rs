/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RendererId(u64);

impl RendererId {
	pub const fn from_raw(raw: u64) -> Self {
		Self(raw)
	}

	pub const fn as_raw(self) -> u64 {
		self.0
	}
}