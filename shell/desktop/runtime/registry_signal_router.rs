/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use futures_util::stream::{self, BoxStream};
use graphshell_core::signal_router::{
    LifecycleSignal as PortableLifecycleSignal, RegistrySignal as PortableRegistrySignal,
    SignalEnvelope as PortableSignalEnvelope, SignalRouter, SignalTopic as PortableSignalTopic,
};

use crate::shell::desktop::runtime::registries::phase3_subscribe_signal_async;
use crate::shell::desktop::runtime::registries::signal_routing::{
    LifecycleSignal, RegistryEventSignal, SignalKind, SignalTopic,
};

#[derive(Default)]
pub(crate) struct RegistrySignalRouter;

impl SignalRouter for RegistrySignalRouter {
    fn subscribe(&self, topic: PortableSignalTopic) -> BoxStream<'static, PortableSignalEnvelope> {
        match topic {
            PortableSignalTopic::Lifecycle => lifecycle_stream(),
            PortableSignalTopic::RegistryEvent => registry_stream(),
        }
    }
}

fn lifecycle_stream() -> BoxStream<'static, PortableSignalEnvelope> {
    let subscription = phase3_subscribe_signal_async(SignalTopic::Lifecycle);
    Box::pin(stream::unfold(
        subscription,
        |mut subscription| async move {
            loop {
                let signal = subscription.recv().await?;
                if let SignalKind::Lifecycle(LifecycleSignal::SemanticIndexUpdated {
                    indexed_nodes,
                }) = signal.kind
                {
                    return Some((
                        PortableSignalEnvelope::Lifecycle(
                            PortableLifecycleSignal::SemanticIndexUpdated { indexed_nodes },
                        ),
                        subscription,
                    ));
                }
            }
        },
    ))
}

fn registry_stream() -> BoxStream<'static, PortableSignalEnvelope> {
    let subscription = phase3_subscribe_signal_async(SignalTopic::RegistryEvent);
    Box::pin(stream::unfold(
        subscription,
        |mut subscription| async move {
            loop {
                let signal = subscription.recv().await?;
                if let SignalKind::RegistryEvent(registry_signal) = signal.kind
                    && let Some(mapped) = map_registry_signal(registry_signal)
                {
                    return Some((PortableSignalEnvelope::RegistryEvent(mapped), subscription));
                }
            }
        },
    ))
}

fn map_registry_signal(signal: RegistryEventSignal) -> Option<PortableRegistrySignal> {
    match signal {
        RegistryEventSignal::WorkbenchProjectionRefreshRequested { .. } => {
            Some(PortableRegistrySignal::WorkbenchProjectionRefreshRequested)
        }
        RegistryEventSignal::SettingsRouteRequested { url } => {
            Some(PortableRegistrySignal::SettingsRouteRequested { url })
        }
        RegistryEventSignal::ThemeChanged { .. } => Some(PortableRegistrySignal::ThemeChanged),
        RegistryEventSignal::LensChanged { .. } => Some(PortableRegistrySignal::LensChanged),
        RegistryEventSignal::PhysicsProfileChanged { .. } => {
            Some(PortableRegistrySignal::PhysicsProfileChanged)
        }
        RegistryEventSignal::CanvasProfileChanged { .. } => {
            Some(PortableRegistrySignal::CanvasProfileChanged)
        }
        RegistryEventSignal::WorkbenchSurfaceChanged { .. } => {
            Some(PortableRegistrySignal::WorkbenchSurfaceChanged)
        }
        _ => None,
    }
}
