/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use super::context_map::StorageContextBinding;
use super::transition_policy::{BackendTransitionPlan, StorageTransitionClass};

pub(crate) trait StorageInteropCoordinator: Send + Sync {
    fn plan_backend_transition(
        &self,
        binding: &StorageContextBinding,
        target_backend: &str,
    ) -> BackendTransitionPlan;
}

pub(crate) type StorageInteropCoordinatorHandle = Arc<dyn StorageInteropCoordinator>;

#[derive(Debug, Default)]
pub(crate) struct NoopStorageInteropCoordinator;

impl StorageInteropCoordinator for NoopStorageInteropCoordinator {
    fn plan_backend_transition(
        &self,
        binding: &StorageContextBinding,
        target_backend: &str,
    ) -> BackendTransitionPlan {
        let continuity_warning = Some(format!(
            "No interop policy registered for {:?} -> {target_backend}; defaulting to isolated fallback context.",
            binding.backend
        ));

        BackendTransitionPlan {
            target_backend: target_backend.to_string(),
            transition_class: StorageTransitionClass::IsolatedFallbackContext,
            continuity_warning,
        }
    }
}
