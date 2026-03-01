pub(crate) mod action;
pub(crate) mod identity;
pub(crate) mod input;
pub(crate) mod knowledge;
pub(crate) mod lens;
pub(crate) mod physics;
pub(crate) mod protocol;

use std::sync::OnceLock;

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::registries::atomic::ProtocolHandlerProviders;
use crate::registries::atomic::ViewerHandlerProviders;
use crate::registries::atomic::diagnostics;
use crate::registries::atomic::layout::LayoutRegistry;
use crate::registries::atomic::protocol::ProtocolContractRegistry;
use crate::registries::atomic::theme::ThemeRegistry;
use crate::registries::atomic::viewer::{ViewerRegistry, ViewerSelection};
use crate::registries::domain::layout::ConformanceLevel;
use crate::registries::domain::layout::LayoutDomainRegistry;
use crate::registries::domain::layout::viewer_surface::{
    VIEWER_SURFACE_DEFAULT, ViewerSurfaceResolution,
};
use crate::registries::domain::presentation::PresentationDomainRegistry;
use crate::registries::infrastructure::ModRegistry;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use action::{
    ACTION_DETAIL_VIEW_SUBMIT, ACTION_GRAPH_VIEW_SUBMIT, ACTION_OMNIBOX_NODE_SEARCH,
    ACTION_VERSE_FORGET_DEVICE, ACTION_VERSE_PAIR_DEVICE, ACTION_VERSE_SHARE_WORKSPACE,
    ACTION_VERSE_SYNC_NOW, ActionPayload, ActionRegistry, PairingMode,
};
use diagnostics::DiagnosticsRegistry;
use identity::IdentityRegistry;
use input::{INPUT_BINDING_TOOLBAR_SUBMIT, InputRegistry};
use knowledge::KnowledgeRegistry;
use lens::LensRegistry;
use physics::PhysicsRegistry;
use protocol::{
    ProtocolRegistry, ProtocolResolution, ProtocolResolveControl, ProtocolResolveOutcome,
};
use servo::ServoUrl;

pub(crate) const CHANNEL_PROTOCOL_RESOLVE_STARTED: &str = "registry.protocol.resolve_started";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED: &str = "registry.protocol.resolve_succeeded";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_FAILED: &str = "registry.protocol.resolve_failed";
pub(crate) const CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED: &str = "registry.protocol.fallback_used";
pub(crate) const CHANNEL_VIEWER_SELECT_STARTED: &str = "registry.viewer.select_started";
pub(crate) const CHANNEL_VIEWER_SELECT_SUCCEEDED: &str = "registry.viewer.select_succeeded";
pub(crate) const CHANNEL_VIEWER_FALLBACK_USED: &str = "registry.viewer.fallback_used";
pub(crate) const CHANNEL_VIEWER_CAPABILITY_PARTIAL: &str = "registry.viewer.capability_partial";
pub(crate) const CHANNEL_VIEWER_CAPABILITY_NONE: &str = "registry.viewer.capability_none";
pub(crate) const CHANNEL_SURFACE_CONFORMANCE_PARTIAL: &str = "registry.surface.conformance_partial";
pub(crate) const CHANNEL_SURFACE_CONFORMANCE_NONE: &str = "registry.surface.conformance_none";
pub(crate) const CHANNEL_ACTION_EXECUTE_STARTED: &str = "registry.action.execute_started";
pub(crate) const CHANNEL_ACTION_EXECUTE_SUCCEEDED: &str = "registry.action.execute_succeeded";
pub(crate) const CHANNEL_ACTION_EXECUTE_FAILED: &str = "registry.action.execute_failed";
pub(crate) const CHANNEL_INPUT_BINDING_RESOLVED: &str = "registry.input.binding_resolved";
pub(crate) const CHANNEL_INPUT_BINDING_MISSING: &str = "registry.input.binding_missing";
pub(crate) const CHANNEL_INPUT_BINDING_CONFLICT: &str = "registry.input.binding_conflict";
pub(crate) const CHANNEL_LENS_RESOLVE_SUCCEEDED: &str = "registry.lens.resolve_succeeded";
pub(crate) const CHANNEL_LENS_RESOLVE_FAILED: &str = "registry.lens.resolve_failed";
pub(crate) const CHANNEL_LENS_FALLBACK_USED: &str = "registry.lens.fallback_used";
pub(crate) const CHANNEL_LAYOUT_LOOKUP_SUCCEEDED: &str = "registry.layout.lookup_succeeded";
pub(crate) const CHANNEL_LAYOUT_LOOKUP_FAILED: &str = "registry.layout.lookup_failed";
pub(crate) const CHANNEL_LAYOUT_FALLBACK_USED: &str = "registry.layout.fallback_used";
pub(crate) const CHANNEL_THEME_LOOKUP_SUCCEEDED: &str = "registry.theme.lookup_succeeded";
pub(crate) const CHANNEL_THEME_LOOKUP_FAILED: &str = "registry.theme.lookup_failed";
pub(crate) const CHANNEL_THEME_FALLBACK_USED: &str = "registry.theme.fallback_used";
pub(crate) const CHANNEL_PHYSICS_LOOKUP_SUCCEEDED: &str = "registry.physics.lookup_succeeded";
pub(crate) const CHANNEL_PHYSICS_LOOKUP_FAILED: &str = "registry.physics.lookup_failed";
pub(crate) const CHANNEL_PHYSICS_FALLBACK_USED: &str = "registry.physics.fallback_used";
pub(crate) const CHANNEL_IDENTITY_SIGN_STARTED: &str = "registry.identity.sign_started";
pub(crate) const CHANNEL_IDENTITY_SIGN_SUCCEEDED: &str = "registry.identity.sign_succeeded";
pub(crate) const CHANNEL_IDENTITY_SIGN_FAILED: &str = "registry.identity.sign_failed";
pub(crate) const CHANNEL_IDENTITY_KEY_UNAVAILABLE: &str = "registry.identity.key_unavailable";
pub(crate) const CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED: &str =
    "registry.diagnostics.channel_registered";
pub(crate) const CHANNEL_DIAGNOSTICS_CONFIG_CHANGED: &str = "registry.diagnostics.config_changed";
pub(crate) const CHANNEL_INVARIANT_TIMEOUT: &str = "registry.invariant.timeout";
pub(crate) const CHANNEL_MOD_LOAD_STARTED: &str = "registry.mod.load_started";
pub(crate) const CHANNEL_MOD_LOAD_SUCCEEDED: &str = "registry.mod.load_succeeded";
pub(crate) const CHANNEL_MOD_LOAD_FAILED: &str = "registry.mod.load_failed";
pub(crate) const CHANNEL_MOD_DEPENDENCY_MISSING: &str = "registry.mod.dependency_missing";
pub(crate) const CHANNEL_STARTUP_CONFIG_SNAPSHOT: &str = "startup.config.snapshot";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED: &str =
    "startup.persistence.open_started";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED: &str =
    "startup.persistence.open_succeeded";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED: &str = "startup.persistence.open_failed";
pub(crate) const CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT: &str =
    "startup.persistence.open_timeout";
pub(crate) const CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED: &str = "persistence.recover.succeeded";
pub(crate) const CHANNEL_PERSISTENCE_RECOVER_FAILED: &str = "persistence.recover.failed";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_MODE: &str = "startup.verse.init_mode";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_SUCCEEDED: &str = "startup.verse.init_succeeded";
pub(crate) const CHANNEL_STARTUP_VERSE_INIT_FAILED: &str = "startup.verse.init_failed";
pub(crate) const CHANNEL_UI_HISTORY_MANAGER_LIMIT: &str = "ui.history_manager.limit_applied";
pub(crate) const CHANNEL_UI_CLIPBOARD_COPY_FAILED: &str = "ui.clipboard.copy_failed";
pub(crate) const CHANNEL_VERSE_PREINIT_CALL: &str = "verse.preinit.call";
pub(crate) const CHANNEL_VERSE_SYNC_UNIT_SENT: &str = "verse.sync.unit_sent";
pub(crate) const CHANNEL_VERSE_SYNC_UNIT_RECEIVED: &str = "verse.sync.unit_received";
pub(crate) const CHANNEL_VERSE_SYNC_INTENT_APPLIED: &str = "verse.sync.intent_applied";
pub(crate) const CHANNEL_VERSE_SYNC_ACCESS_DENIED: &str = "verse.sync.access_denied";
pub(crate) const CHANNEL_VERSE_SYNC_CONNECTION_REJECTED: &str = "verse.sync.connection_rejected";
pub(crate) const CHANNEL_VERSE_SYNC_IDENTITY_GENERATED: &str = "verse.sync.identity_generated";
pub(crate) const CHANNEL_VERSE_SYNC_CONFLICT_DETECTED: &str = "verse.sync.conflict_detected";
pub(crate) const CHANNEL_VERSE_SYNC_CONFLICT_RESOLVED: &str = "verse.sync.conflict_resolved";
pub(crate) const CHANNEL_COMPOSITOR_GL_STATE_VIOLATION: &str = "compositor.gl_state_violation";
pub(crate) const CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED: &str =
    "compositor.focus_activation.deferred";
pub(crate) const CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED: &str =
    "semantic.intent.create_new_webview_unmapped";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE: &str =
    "compositor.overlay.style.rect_stroke";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY: &str =
    "compositor.overlay.style.chrome_only";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE: &str =
    "compositor.overlay.mode.composited_texture";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY: &str =
    "compositor.overlay.mode.native_overlay";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI: &str =
    "compositor.overlay.mode.embedded_egui";
pub(crate) const CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER: &str =
    "compositor.overlay.mode.placeholder";
pub(crate) const CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED: &str =
    "compositor.replay.sample_recorded";
pub(crate) const CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED: &str =
    "compositor.replay.artifact_recorded";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED: &str =
    "compositor.differential.content_composed";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED: &str =
    "compositor.differential.content_skipped";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE: &str =
    "compositor.differential.fallback_no_prior_signature";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED: &str =
    "compositor.differential.fallback_signature_changed";
pub(crate) const CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE: &str =
    "compositor.differential.skip_rate_basis_points";
pub(crate) const CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT: &str =
    "compositor.content.culled_offviewport";
pub(crate) const CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE: &str =
    "compositor.degradation.gpu_pressure";
pub(crate) const CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE: &str =
    "compositor.degradation.placeholder_mode";

static REGISTRY_RUNTIME: OnceLock<RegistryRuntime> = OnceLock::new();

fn runtime() -> &'static RegistryRuntime {
    REGISTRY_RUNTIME.get_or_init(RegistryRuntime::new_with_mods)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Phase0NavigationDecision {
    pub(crate) normalized_url: ServoUrl,
    pub(crate) protocol: ProtocolResolution,
    pub(crate) viewer: ViewerSelection,
}

#[derive(Default)]
pub(crate) struct RegistryRuntime {
    action: ActionRegistry,
    #[allow(dead_code)]
    pub(crate) diagnostics: DiagnosticsRegistry,
    #[allow(dead_code)]
    identity: IdentityRegistry,
    input: InputRegistry,
    layout: LayoutRegistry,
    lens: LensRegistry,
    #[allow(dead_code)]
    physics: PhysicsRegistry,
    protocol: ProtocolRegistry,
    #[allow(dead_code)]
    theme: ThemeRegistry,
    viewer: ViewerRegistry,
    pub(crate) knowledge: KnowledgeRegistry,
}

#[allow(dead_code)]
pub(crate) fn phase3_sign_identity_payload(identity_id: &str, payload: &[u8]) -> Option<String> {
    debug_assert!(!diagnostics::phase3_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_IDENTITY_SIGN_STARTED,
        byte_len: identity_id.len().saturating_add(payload.len()),
    });

    let result = runtime().identity.sign(identity_id, payload);
    if result.succeeded {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_IDENTITY_SIGN_SUCCEEDED,
            latency_us: 1,
        });
    } else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_IDENTITY_SIGN_FAILED,
            latency_us: 1,
        });
    }

    if !result.resolution.key_available {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_IDENTITY_KEY_UNAVAILABLE,
            byte_len: result.resolution.resolved_id.len(),
        });
    }

    result.signature
}

impl RegistryRuntime {
    fn build_provider_wired_registries(
        protocol_providers: &ProtocolHandlerProviders,
        viewer_providers: &ViewerHandlerProviders,
    ) -> (ProtocolRegistry, ViewerRegistry) {
        let mut protocol_registry = ProtocolRegistry::default();
        let mut protocol_contract_registry = ProtocolContractRegistry::core_seed();
        protocol_providers.apply_all(&mut protocol_contract_registry);
        for scheme in protocol_contract_registry.scheme_ids() {
            protocol_registry.register_scheme(&scheme);
        }

        let mut viewer_registry = ViewerRegistry::default();
        viewer_providers.apply_all(&mut viewer_registry);

        (protocol_registry, viewer_registry)
    }

    #[cfg(test)]
    fn new_with_provider_registries_for_tests(
        protocol_providers: ProtocolHandlerProviders,
        viewer_providers: ViewerHandlerProviders,
    ) -> Self {
        let (protocol_registry, viewer_registry) =
            Self::build_provider_wired_registries(&protocol_providers, &viewer_providers);
        Self {
            action: ActionRegistry::default(),
            diagnostics: DiagnosticsRegistry::default(),
            identity: IdentityRegistry::default(),
            input: InputRegistry::default(),
            layout: LayoutRegistry::default(),
            lens: LensRegistry::default(),
            physics: PhysicsRegistry::default(),
            protocol: protocol_registry,
            theme: ThemeRegistry::default(),
            viewer: viewer_registry,
            knowledge: KnowledgeRegistry::default(),
        }
    }

    /// Create a new RegistryRuntime with mods discovered and their handlers registered.
    /// This is the standard way to initialize registries during app startup (Phase 2.4).
    ///
    /// Provider wiring for protocol/viewer paths is applied into the runtime
    /// registries returned by this constructor.
    #[allow(dead_code)]
    pub(crate) fn new_with_mods() -> Self {
        // Discover and resolve mod dependencies
        let mut mod_registry = ModRegistry::new();
        if let Err(e) = mod_registry.resolve_dependencies() {
            log::error!(
                "Failed to resolve mod dependencies: {:?}. Using core seed only.",
                e
            );
        }
        let _loaded_mods = mod_registry.load_all();

        // Wire up handler providers from active mods.
        let mut protocol_providers = ProtocolHandlerProviders::new();
        let mut viewer_providers = ViewerHandlerProviders::new();

        // Register handlers from active mods
        if mod_registry.get_status("mod:verso").is_some() {
            crate::mods::verso::register_protocol_handlers(&mut protocol_providers);
            crate::mods::verso::register_viewer_handlers(&mut viewer_providers);
            log::debug!("registries: verso mod handlers registered into provider registries");
        }

        if mod_registry.get_status("mod:verse").is_some()
            || mod_registry.get_status("verse").is_some()
        {
            crate::mods::verse::register_protocol_handlers(&mut protocol_providers);
            log::debug!("registries: verse mod handlers registered into provider registries");
        }

        let (protocol_registry, viewer_registry) =
            Self::build_provider_wired_registries(&protocol_providers, &viewer_providers);

        // Create the RegistryRuntime with provider-wired registries.
        Self {
            action: ActionRegistry::default(),
            diagnostics: DiagnosticsRegistry::default(),
            identity: IdentityRegistry::default(),
            input: InputRegistry::default(),
            layout: LayoutRegistry::default(),
            lens: LensRegistry::default(),
            physics: PhysicsRegistry::default(),
            protocol: protocol_registry,
            theme: ThemeRegistry::default(),
            viewer: viewer_registry,
            knowledge: KnowledgeRegistry::default(),
        }
    }

    pub(crate) fn observe_navigation_url_with_control(
        &self,
        uri: &str,
        mime_hint: Option<&str>,
        control: ProtocolResolveControl,
    ) -> Option<(ProtocolResolution, ViewerSelection)> {
        debug_assert!(!diagnostics::phase0_required_channels().is_empty());
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_PROTOCOL_RESOLVE_STARTED,
            byte_len: uri.len(),
        });
        let protocol = match self.protocol.resolve_with_control(uri, control) {
            ProtocolResolveOutcome::Resolved(resolution) => resolution,
            ProtocolResolveOutcome::Cancelled => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_PROTOCOL_RESOLVE_FAILED,
                    latency_us: 1,
                });
                return None;
            }
        };

        if protocol.supported {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED,
                latency_us: 1,
            });
        } else {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_FAILED,
                latency_us: 1,
            });
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED,
                byte_len: protocol.matched_scheme.len(),
            });
        }

        let effective_mime_hint = mime_hint.or(protocol.inferred_mime_hint.as_deref());

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_VIEWER_SELECT_STARTED,
            byte_len: effective_mime_hint.unwrap_or(uri).len(),
        });
        let viewer = self.viewer.select_for_uri(uri, effective_mime_hint);
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_VIEWER_SELECT_SUCCEEDED,
            latency_us: 1,
        });
        emit_viewer_capability_diagnostics(&viewer);
        if viewer.fallback_used {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_VIEWER_FALLBACK_USED,
                byte_len: viewer.viewer_id.len(),
            });
        }

        Some((protocol, viewer))
    }
}

pub(crate) fn phase2_resolve_toolbar_submit_binding() -> bool {
    phase2_resolve_input_binding(INPUT_BINDING_TOOLBAR_SUBMIT)
}

pub(crate) fn phase2_resolve_input_binding(binding_id: &str) -> bool {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    let resolution = runtime().input.resolve(binding_id);

    if resolution.matched {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_INPUT_BINDING_RESOLVED,
            byte_len: resolution.action_id.as_deref().unwrap_or_default().len(),
        });
        return true;
    }

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_INPUT_BINDING_MISSING,
        byte_len: resolution.binding_id.len(),
    });
    false
}

pub(crate) fn phase2_resolve_lens(lens_id: &str) -> crate::app::LensConfig {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    let runtime = runtime();
    let resolution = runtime.lens.resolve(lens_id);
    log::debug!(
        "registry lens resolve requested='{}' resolved='{}' matched={} fallback={}",
        resolution.requested_id,
        resolution.resolved_id,
        resolution.matched,
        resolution.fallback_used
    );

    if resolution.matched {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_LENS_RESOLVE_SUCCEEDED,
            latency_us: 1,
        });
    } else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_LENS_RESOLVE_FAILED,
            latency_us: 1,
        });
    }

    if resolution.fallback_used {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_LENS_FALLBACK_USED,
            byte_len: resolution.resolved_id.len(),
        });
    }

    let presentation_domain = PresentationDomainRegistry::default();
    let presentation_resolution = presentation_domain.resolve_profile(
        &resolution.definition.physics_id,
        resolution
            .definition
            .theme_id
            .as_deref()
            .unwrap_or(crate::registries::atomic::theme::THEME_ID_DEFAULT),
    );
    let physics_resolution = presentation_resolution.physics;
    emit_lookup_diagnostics(
        physics_resolution.matched,
        physics_resolution.fallback_used,
        CHANNEL_PHYSICS_LOOKUP_SUCCEEDED,
        CHANNEL_PHYSICS_LOOKUP_FAILED,
        CHANNEL_PHYSICS_FALLBACK_USED,
        &physics_resolution.resolved_id,
    );

    let layout_resolution = runtime.layout.resolve(&resolution.definition.layout_id);
    emit_lookup_diagnostics(
        layout_resolution.matched,
        layout_resolution.fallback_used,
        CHANNEL_LAYOUT_LOOKUP_SUCCEEDED,
        CHANNEL_LAYOUT_LOOKUP_FAILED,
        CHANNEL_LAYOUT_FALLBACK_USED,
        &layout_resolution.resolved_id,
    );

    let theme_resolution = Some(presentation_resolution.theme);
    if let Some(theme_resolution) = &theme_resolution {
        emit_lookup_diagnostics(
            theme_resolution.matched,
            theme_resolution.fallback_used,
            CHANNEL_THEME_LOOKUP_SUCCEEDED,
            CHANNEL_THEME_LOOKUP_FAILED,
            CHANNEL_THEME_FALLBACK_USED,
            &theme_resolution.resolved_id,
        );
    }

    crate::app::LensConfig {
        name: resolution.definition.display_name,
        lens_id: Some(resolution.resolved_id),
        physics_id: Some(physics_resolution.resolved_id),
        layout_id: Some(layout_resolution.resolved_id),
        theme_id: theme_resolution
            .as_ref()
            .map(|resolved| resolved.resolved_id.clone()),
        physics: physics_resolution.profile,
        layout: layout_resolution.layout,
        theme: theme_resolution.map(|resolved| resolved.theme_id),
        filters: resolution.definition.filters,
    }
}

pub(crate) fn phase2_resolve_lens_components(
    lens: &crate::app::LensConfig,
) -> crate::app::LensConfig {
    let has_component_ids =
        lens.physics_id.is_some() || lens.layout_id.is_some() || lens.theme_id.is_some();
    if !has_component_ids {
        return lens.clone();
    }

    let runtime = runtime();
    let presentation_domain = PresentationDomainRegistry::default();
    let mut normalized = lens.clone();

    if let Some(physics_id) = lens.physics_id.as_deref() {
        let physics_resolution = presentation_domain
            .resolve_profile(
                physics_id,
                lens.theme_id
                    .as_deref()
                    .unwrap_or(crate::registries::atomic::theme::THEME_ID_DEFAULT),
            )
            .physics;
        emit_lookup_diagnostics(
            physics_resolution.matched,
            physics_resolution.fallback_used,
            CHANNEL_PHYSICS_LOOKUP_SUCCEEDED,
            CHANNEL_PHYSICS_LOOKUP_FAILED,
            CHANNEL_PHYSICS_FALLBACK_USED,
            &physics_resolution.resolved_id,
        );
        normalized.physics = physics_resolution.profile;
        normalized.physics_id = Some(physics_resolution.resolved_id);
    }

    if let Some(layout_id) = lens.layout_id.as_deref() {
        let layout_resolution = runtime.layout.resolve(layout_id);
        emit_lookup_diagnostics(
            layout_resolution.matched,
            layout_resolution.fallback_used,
            CHANNEL_LAYOUT_LOOKUP_SUCCEEDED,
            CHANNEL_LAYOUT_LOOKUP_FAILED,
            CHANNEL_LAYOUT_FALLBACK_USED,
            &layout_resolution.resolved_id,
        );
        normalized.layout = layout_resolution.layout;
        normalized.layout_id = Some(layout_resolution.resolved_id);
    }

    if let Some(theme_id) = lens.theme_id.as_deref() {
        let theme_resolution = presentation_domain
            .resolve_profile(
                lens.physics_id.as_deref().unwrap_or(
                    crate::shell::desktop::runtime::registries::physics::PHYSICS_ID_DEFAULT,
                ),
                theme_id,
            )
            .theme;
        emit_lookup_diagnostics(
            theme_resolution.matched,
            theme_resolution.fallback_used,
            CHANNEL_THEME_LOOKUP_SUCCEEDED,
            CHANNEL_THEME_LOOKUP_FAILED,
            CHANNEL_THEME_FALLBACK_USED,
            &theme_resolution.resolved_id,
        );
        normalized.theme = Some(theme_resolution.theme_id.clone());
        normalized.theme_id = Some(theme_resolution.resolved_id);
    }

    normalized
}

fn emit_lookup_diagnostics(
    matched: bool,
    fallback_used: bool,
    success_channel: &'static str,
    failed_channel: &'static str,
    fallback_channel: &'static str,
    resolved_id: &str,
) {
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if matched {
            success_channel
        } else {
            failed_channel
        },
        latency_us: 1,
    });

    if fallback_used {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: fallback_channel,
            byte_len: resolved_id.len(),
        });
    }
}

fn emit_viewer_capability_diagnostics(viewer: &ViewerSelection) {
    for (level, reason) in [
        (
            &viewer.capabilities.accessibility.level,
            viewer.capabilities.accessibility.reason.as_deref(),
        ),
        (
            &viewer.capabilities.security.level,
            viewer.capabilities.security.reason.as_deref(),
        ),
        (
            &viewer.capabilities.storage.level,
            viewer.capabilities.storage.reason.as_deref(),
        ),
        (
            &viewer.capabilities.history.level,
            viewer.capabilities.history.reason.as_deref(),
        ),
    ] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_VIEWER_CAPABILITY_PARTIAL,
                    byte_len: reason.unwrap_or_default().len(),
                });
            }
            ConformanceLevel::None => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_VIEWER_CAPABILITY_NONE,
                    byte_len: reason.unwrap_or_default().len(),
                });
            }
        }
    }
}

fn emit_surface_conformance_diagnostics(
    accessibility: ConformanceLevel,
    security: ConformanceLevel,
    storage: ConformanceLevel,
    history: ConformanceLevel,
) {
    for level in [accessibility, security, storage, history] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SURFACE_CONFORMANCE_PARTIAL,
                    byte_len: 1,
                });
            }
            ConformanceLevel::None => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SURFACE_CONFORMANCE_NONE,
                    byte_len: 1,
                });
            }
        }
    }
}

pub(crate) fn phase2_execute_omnibox_node_search_action(
    app: &GraphBrowserApp,
    query: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: query.len(),
    });

    let execution = runtime().action.execute(
        ACTION_OMNIBOX_NODE_SEARCH,
        app,
        ActionPayload::OmniboxNodeSearch {
            query: query.to_string(),
        },
    );

    log::debug!(
        "registry action '{}' executed for omnibox query '{}'; succeeded={} intents={}",
        execution.action_id,
        query,
        execution.succeeded,
        execution.intents.len()
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    execution.intents
}

pub(crate) fn phase5_execute_verse_sync_now_action(app: &GraphBrowserApp) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution =
        runtime()
            .action
            .execute(ACTION_VERSE_SYNC_NOW, app, ActionPayload::VerseSyncNow);
    execution.intents
}

pub(crate) fn phase5_execute_verse_pair_local_peer_action(
    app: &GraphBrowserApp,
    node_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_PAIR_DEVICE,
        app,
        ActionPayload::VersePairDevice {
            mode: PairingMode::LocalPeer {
                node_id: node_id.to_string(),
            },
        },
    );
    execution.intents
}

pub(crate) fn phase5_execute_verse_pair_code_action(
    app: &GraphBrowserApp,
    code: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_PAIR_DEVICE,
        app,
        ActionPayload::VersePairDevice {
            mode: PairingMode::EnterCode {
                code: code.to_string(),
            },
        },
    );
    execution.intents
}

pub(crate) fn phase5_execute_verse_share_workspace_action(
    app: &GraphBrowserApp,
    workspace_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_SHARE_WORKSPACE,
        app,
        ActionPayload::VerseShareWorkspace {
            workspace_id: workspace_id.to_string(),
        },
    );
    execution.intents
}

pub(crate) fn phase5_execute_verse_forget_device_action(
    app: &GraphBrowserApp,
    node_id: &str,
) -> Vec<GraphIntent> {
    debug_assert!(!diagnostics::phase5_required_channels().is_empty());
    let execution = runtime().action.execute(
        ACTION_VERSE_FORGET_DEVICE,
        app,
        ActionPayload::VerseForgetDevice {
            node_id: node_id.to_string(),
        },
    );
    execution.intents
}

#[allow(dead_code)]
pub(crate) fn register_mod_diagnostics_channel(
    mod_id: &str,
    channel_id: &str,
    schema_version: u16,
    description: Option<String>,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    let created = diagnostics::register_mod_channel_global(
        mod_id,
        channel_id,
        schema_version,
        description,
        &[diagnostics::DiagnosticsCapability::RegisterChannels],
    )?;
    if created {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED,
            byte_len: channel_id.len(),
        });
    }
    Ok(created)
}

#[allow(dead_code)]
pub(crate) fn register_verse_diagnostics_channel(
    peer_id: &str,
    channel_id: &str,
    schema_version: u16,
    description: Option<String>,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    let created = diagnostics::register_verse_channel_global(
        peer_id,
        channel_id,
        schema_version,
        description,
        &[diagnostics::DiagnosticsCapability::RegisterChannels],
    )?;
    if created {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_DIAGNOSTICS_CHANNEL_REGISTERED,
            byte_len: channel_id.len(),
        });
    }
    Ok(created)
}

#[allow(dead_code)]
pub(crate) fn register_diagnostics_invariant(
    invariant: diagnostics::DiagnosticsInvariant,
) -> Result<bool, diagnostics::ChannelRegistrationError> {
    diagnostics::register_invariant_global(
        invariant,
        &[diagnostics::DiagnosticsCapability::RegisterInvariants],
    )
}

#[cfg(test)]
pub(crate) fn phase3_sign_identity_payload_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    identity_id: &str,
    payload: &[u8],
) -> Option<String> {
    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_IDENTITY_SIGN_STARTED,
        identity_id.len().saturating_add(payload.len()),
    );

    let result = RegistryRuntime::default()
        .identity
        .sign(identity_id, payload);
    diagnostics_state.emit_message_received_for_tests(
        if result.succeeded {
            CHANNEL_IDENTITY_SIGN_SUCCEEDED
        } else {
            CHANNEL_IDENTITY_SIGN_FAILED
        },
        1,
    );

    if !result.resolution.key_available {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_IDENTITY_KEY_UNAVAILABLE,
            result.resolution.resolved_id.len(),
        );
    }

    result.signature
}

#[cfg(test)]
pub(crate) fn phase2_resolve_toolbar_submit_binding_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) -> bool {
    phase2_resolve_input_binding_for_tests(diagnostics_state, INPUT_BINDING_TOOLBAR_SUBMIT)
}

#[cfg(test)]
pub(crate) fn phase2_resolve_input_binding_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    binding_id: &str,
) -> bool {
    let resolution = RegistryRuntime::default().input.resolve(binding_id);

    if resolution.matched {
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_INPUT_BINDING_RESOLVED,
            resolution.action_id.as_deref().unwrap_or_default().len(),
        );
        return true;
    }

    diagnostics_state
        .emit_message_sent_for_tests(CHANNEL_INPUT_BINDING_MISSING, resolution.binding_id.len());
    false
}

#[cfg(test)]
pub(crate) fn phase2_resolve_lens_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    lens_id: &str,
) -> crate::app::LensConfig {
    let runtime = RegistryRuntime::default();
    let resolution = runtime.lens.resolve(lens_id);

    if resolution.matched {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_SUCCEEDED, 1);
    } else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_LENS_RESOLVE_FAILED, 1);
    }

    if resolution.fallback_used {
        diagnostics_state
            .emit_message_sent_for_tests(CHANNEL_LENS_FALLBACK_USED, resolution.resolved_id.len());
    }

    let presentation_domain = PresentationDomainRegistry::default();
    let presentation_resolution = presentation_domain.resolve_profile(
        &resolution.definition.physics_id,
        resolution
            .definition
            .theme_id
            .as_deref()
            .unwrap_or(crate::registries::atomic::theme::THEME_ID_DEFAULT),
    );
    let physics_resolution = presentation_resolution.physics;
    emit_lookup_diagnostics_for_tests(
        diagnostics_state,
        physics_resolution.matched,
        physics_resolution.fallback_used,
        CHANNEL_PHYSICS_LOOKUP_SUCCEEDED,
        CHANNEL_PHYSICS_LOOKUP_FAILED,
        CHANNEL_PHYSICS_FALLBACK_USED,
        &physics_resolution.resolved_id,
    );

    let layout_resolution = runtime.layout.resolve(&resolution.definition.layout_id);
    emit_lookup_diagnostics_for_tests(
        diagnostics_state,
        layout_resolution.matched,
        layout_resolution.fallback_used,
        CHANNEL_LAYOUT_LOOKUP_SUCCEEDED,
        CHANNEL_LAYOUT_LOOKUP_FAILED,
        CHANNEL_LAYOUT_FALLBACK_USED,
        &layout_resolution.resolved_id,
    );

    let theme_resolution = Some(presentation_resolution.theme);
    if let Some(theme_resolution) = &theme_resolution {
        emit_lookup_diagnostics_for_tests(
            diagnostics_state,
            theme_resolution.matched,
            theme_resolution.fallback_used,
            CHANNEL_THEME_LOOKUP_SUCCEEDED,
            CHANNEL_THEME_LOOKUP_FAILED,
            CHANNEL_THEME_FALLBACK_USED,
            &theme_resolution.resolved_id,
        );
    }

    crate::app::LensConfig {
        name: resolution.definition.display_name,
        lens_id: Some(resolution.resolved_id),
        physics_id: Some(physics_resolution.resolved_id),
        layout_id: Some(layout_resolution.resolved_id),
        theme_id: theme_resolution
            .as_ref()
            .map(|resolved| resolved.resolved_id.clone()),
        physics: physics_resolution.profile,
        layout: layout_resolution.layout,
        theme: theme_resolution.map(|resolved| resolved.theme_id),
        filters: resolution.definition.filters,
    }
}

#[cfg(test)]
pub(crate) fn phase2_resolve_lens_components_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    lens: &crate::app::LensConfig,
) -> crate::app::LensConfig {
    let has_component_ids =
        lens.physics_id.is_some() || lens.layout_id.is_some() || lens.theme_id.is_some();
    if !has_component_ids {
        return lens.clone();
    }

    let runtime = RegistryRuntime::default();
    let presentation_domain = PresentationDomainRegistry::default();
    let mut normalized = lens.clone();

    if let Some(physics_id) = lens.physics_id.as_deref() {
        let physics_resolution = presentation_domain
            .resolve_profile(
                physics_id,
                lens.theme_id
                    .as_deref()
                    .unwrap_or(crate::registries::atomic::theme::THEME_ID_DEFAULT),
            )
            .physics;
        emit_lookup_diagnostics_for_tests(
            diagnostics_state,
            physics_resolution.matched,
            physics_resolution.fallback_used,
            CHANNEL_PHYSICS_LOOKUP_SUCCEEDED,
            CHANNEL_PHYSICS_LOOKUP_FAILED,
            CHANNEL_PHYSICS_FALLBACK_USED,
            &physics_resolution.resolved_id,
        );
        normalized.physics = physics_resolution.profile;
        normalized.physics_id = Some(physics_resolution.resolved_id);
    }

    if let Some(layout_id) = lens.layout_id.as_deref() {
        let layout_resolution = runtime.layout.resolve(layout_id);
        emit_lookup_diagnostics_for_tests(
            diagnostics_state,
            layout_resolution.matched,
            layout_resolution.fallback_used,
            CHANNEL_LAYOUT_LOOKUP_SUCCEEDED,
            CHANNEL_LAYOUT_LOOKUP_FAILED,
            CHANNEL_LAYOUT_FALLBACK_USED,
            &layout_resolution.resolved_id,
        );
        normalized.layout = layout_resolution.layout;
        normalized.layout_id = Some(layout_resolution.resolved_id);
    }

    if let Some(theme_id) = lens.theme_id.as_deref() {
        let theme_resolution = presentation_domain
            .resolve_profile(
                lens.physics_id.as_deref().unwrap_or(
                    crate::shell::desktop::runtime::registries::physics::PHYSICS_ID_DEFAULT,
                ),
                theme_id,
            )
            .theme;
        emit_lookup_diagnostics_for_tests(
            diagnostics_state,
            theme_resolution.matched,
            theme_resolution.fallback_used,
            CHANNEL_THEME_LOOKUP_SUCCEEDED,
            CHANNEL_THEME_LOOKUP_FAILED,
            CHANNEL_THEME_FALLBACK_USED,
            &theme_resolution.resolved_id,
        );
        normalized.theme = Some(theme_resolution.theme_id.clone());
        normalized.theme_id = Some(theme_resolution.resolved_id);
    }

    normalized
}

#[cfg(test)]
fn emit_lookup_diagnostics_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    matched: bool,
    fallback_used: bool,
    success_channel: &'static str,
    failed_channel: &'static str,
    fallback_channel: &'static str,
    resolved_id: &str,
) {
    diagnostics_state.emit_message_received_for_tests(
        if matched {
            success_channel
        } else {
            failed_channel
        },
        1,
    );

    if fallback_used {
        diagnostics_state.emit_message_sent_for_tests(fallback_channel, resolved_id.len());
    }
}

pub(crate) fn phase2_execute_graph_view_submit_action(
    app: &GraphBrowserApp,
    input: &str,
) -> (bool, Vec<GraphIntent>) {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: input.len(),
    });

    let execution = runtime().action.execute(
        ACTION_GRAPH_VIEW_SUBMIT,
        app,
        ActionPayload::GraphViewSubmit {
            input: input.to_string(),
        },
    );

    log::debug!(
        "registry action '{}' executed for graph-view submit '{}'; succeeded={} intents={}",
        execution.action_id,
        input,
        execution.succeeded,
        execution.intents.len()
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    let open_selected_tile = execution.succeeded && !execution.intents.is_empty();
    (open_selected_tile, execution.intents)
}

pub(crate) fn phase2_execute_detail_view_submit_action(
    app: &GraphBrowserApp,
    normalized_url: &str,
    focused_node: Option<crate::graph::NodeKey>,
) -> (bool, Vec<GraphIntent>) {
    debug_assert!(!diagnostics::phase2_required_channels().is_empty());

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_ACTION_EXECUTE_STARTED,
        byte_len: normalized_url.len(),
    });

    let execution = runtime().action.execute(
        ACTION_DETAIL_VIEW_SUBMIT,
        app,
        ActionPayload::DetailViewSubmit {
            normalized_url: normalized_url.to_string(),
            focused_node,
        },
    );

    log::debug!(
        "registry action '{}' executed for detail-view submit '{}'; succeeded={} intents={}",
        execution.action_id,
        normalized_url,
        execution.succeeded,
        execution.intents.len()
    );

    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        latency_us: 1,
    });

    let open_selected_tile = execution
        .intents
        .iter()
        .any(|intent| matches!(intent, GraphIntent::CreateNodeAtUrl { .. }));
    (open_selected_tile, execution.intents)
}

fn phase0_observe_navigation_url_with_control(
    uri: &str,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<(ProtocolResolution, ViewerSelection)> {
    runtime().observe_navigation_url_with_control(uri, mime_hint, control)
}

fn apply_phase0_protocol_policy(parsed_url: ServoUrl, resolution: &ProtocolResolution) -> ServoUrl {
    if resolution.supported || !resolution.fallback_used || resolution.matched_scheme.is_empty() {
        return parsed_url;
    }

    if let Some((_, remainder)) = parsed_url.as_str().split_once(':') {
        let rewritten = format!("{}:{}", resolution.matched_scheme, remainder);
        if let Ok(rewritten_url) = ServoUrl::parse(&rewritten) {
            return rewritten_url;
        }
    }

    parsed_url
}

pub(crate) fn phase0_decide_navigation_with_control(
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<Phase0NavigationDecision> {
    let (protocol_resolution, viewer_selection) =
        phase0_observe_navigation_url_with_control(parsed_url.as_str(), mime_hint, control)?;
    if viewer_selection.viewer_id != "viewer:webview" {
        log::debug!(
            "registry viewer '{}' selected for {}; keeping webview path in Phase 0",
            viewer_selection.viewer_id,
            parsed_url.as_str()
        );
    }

    let normalized_url = apply_phase0_protocol_policy(parsed_url, &protocol_resolution);
    Some(Phase0NavigationDecision {
        normalized_url,
        protocol: protocol_resolution,
        viewer: viewer_selection,
    })
}

pub(crate) fn phase3_resolve_viewer_surface_profile(_viewer_id: &str) -> ViewerSurfaceResolution {
    let layout_domain = LayoutDomainRegistry::default();
    let profile_resolution = layout_domain.resolve_profile(
        crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
        crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );

    let resolution = profile_resolution.viewer_surface;
    emit_surface_conformance_diagnostics(
        profile_resolution
            .canvas
            .profile
            .subsystems
            .accessibility
            .level,
        profile_resolution.canvas.profile.subsystems.security.level,
        profile_resolution.canvas.profile.subsystems.storage.level,
        profile_resolution.canvas.profile.subsystems.history.level,
    );
    emit_surface_conformance_diagnostics(
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .accessibility
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .security
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .storage
            .level,
        profile_resolution
            .workbench_surface
            .profile
            .subsystems
            .history
            .level,
    );
    emit_surface_conformance_diagnostics(
        resolution.profile.subsystems.accessibility.level.clone(),
        resolution.profile.subsystems.security.level.clone(),
        resolution.profile.subsystems.storage.level.clone(),
        resolution.profile.subsystems.history.level.clone(),
    );
    resolution
}

#[cfg(test)]
fn phase0_observe_navigation_url_for_tests_with_control(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    uri: &str,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<(ProtocolResolution, ViewerSelection)> {
    let runtime = RegistryRuntime::default();

    diagnostics_state.emit_message_sent_for_tests(CHANNEL_PROTOCOL_RESOLVE_STARTED, uri.len());
    let protocol = match runtime.protocol.resolve_with_control(uri, control) {
        ProtocolResolveOutcome::Resolved(resolution) => resolution,
        ProtocolResolveOutcome::Cancelled => {
            diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_FAILED, 1);
            return None;
        }
    };
    if protocol.supported {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_SUCCEEDED, 1);
    } else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_PROTOCOL_RESOLVE_FAILED, 1);
        diagnostics_state.emit_message_sent_for_tests(
            CHANNEL_PROTOCOL_RESOLVE_FALLBACK_USED,
            protocol.matched_scheme.len(),
        );
    }

    let effective_mime_hint = mime_hint.or(protocol.inferred_mime_hint.as_deref());

    diagnostics_state.emit_message_sent_for_tests(
        CHANNEL_VIEWER_SELECT_STARTED,
        effective_mime_hint.unwrap_or(uri).len(),
    );
    let viewer = runtime.viewer.select_for_uri(uri, effective_mime_hint);
    diagnostics_state.emit_message_received_for_tests(CHANNEL_VIEWER_SELECT_SUCCEEDED, 1);
    for (level, reason) in [
        (
            &viewer.capabilities.accessibility.level,
            viewer.capabilities.accessibility.reason.as_deref(),
        ),
        (
            &viewer.capabilities.security.level,
            viewer.capabilities.security.reason.as_deref(),
        ),
        (
            &viewer.capabilities.storage.level,
            viewer.capabilities.storage.reason.as_deref(),
        ),
        (
            &viewer.capabilities.history.level,
            viewer.capabilities.history.reason.as_deref(),
        ),
    ] {
        match level {
            ConformanceLevel::Full => {}
            ConformanceLevel::Partial => diagnostics_state.emit_message_sent_for_tests(
                CHANNEL_VIEWER_CAPABILITY_PARTIAL,
                reason.unwrap_or_default().len(),
            ),
            ConformanceLevel::None => diagnostics_state.emit_message_sent_for_tests(
                CHANNEL_VIEWER_CAPABILITY_NONE,
                reason.unwrap_or_default().len(),
            ),
        }
    }
    if viewer.fallback_used {
        diagnostics_state
            .emit_message_sent_for_tests(CHANNEL_VIEWER_FALLBACK_USED, viewer.viewer_id.len());
    }

    Some((protocol, viewer))
}

#[cfg(test)]
pub(crate) fn phase0_decide_navigation_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
) -> Phase0NavigationDecision {
    phase0_decide_navigation_for_tests_with_control(
        diagnostics_state,
        parsed_url,
        mime_hint,
        ProtocolResolveControl::default(),
    )
    .expect("default protocol resolve control must remain active")
}

#[cfg(test)]
pub(crate) fn phase2_execute_omnibox_node_search_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    query: &str,
) -> Vec<GraphIntent> {
    diagnostics_state.emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, query.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_OMNIBOX_NODE_SEARCH,
        app,
        ActionPayload::OmniboxNodeSearch {
            query: query.to_string(),
        },
    );

    log::debug!(
        "registry action '{}' executed in test flow; succeeded={} intents={}",
        execution.action_id,
        execution.succeeded,
        execution.intents.len()
    );

    diagnostics_state.emit_message_received_for_tests(
        if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    execution.intents
}

#[cfg(test)]
pub(crate) fn phase2_execute_graph_view_submit_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    input: &str,
) -> (bool, Vec<GraphIntent>) {
    diagnostics_state.emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, input.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_GRAPH_VIEW_SUBMIT,
        app,
        ActionPayload::GraphViewSubmit {
            input: input.to_string(),
        },
    );

    diagnostics_state.emit_message_received_for_tests(
        if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    let open_selected_tile = execution.succeeded && !execution.intents.is_empty();
    (open_selected_tile, execution.intents)
}

#[cfg(test)]
pub(crate) fn phase2_execute_detail_view_submit_action_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    app: &GraphBrowserApp,
    normalized_url: &str,
    focused_node: Option<crate::graph::NodeKey>,
) -> (bool, Vec<GraphIntent>) {
    diagnostics_state
        .emit_message_sent_for_tests(CHANNEL_ACTION_EXECUTE_STARTED, normalized_url.len());

    let execution = RegistryRuntime::default().action.execute(
        ACTION_DETAIL_VIEW_SUBMIT,
        app,
        ActionPayload::DetailViewSubmit {
            normalized_url: normalized_url.to_string(),
            focused_node,
        },
    );

    diagnostics_state.emit_message_received_for_tests(
        if execution.succeeded {
            CHANNEL_ACTION_EXECUTE_SUCCEEDED
        } else {
            CHANNEL_ACTION_EXECUTE_FAILED
        },
        1,
    );

    let open_selected_tile = execution
        .intents
        .iter()
        .any(|intent| matches!(intent, GraphIntent::CreateNodeAtUrl { .. }));
    (open_selected_tile, execution.intents)
}

#[cfg(test)]
pub(crate) fn phase0_decide_navigation_for_tests_with_control(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    parsed_url: ServoUrl,
    mime_hint: Option<&str>,
    control: ProtocolResolveControl,
) -> Option<Phase0NavigationDecision> {
    let (protocol_resolution, viewer_selection) =
        phase0_observe_navigation_url_for_tests_with_control(
            diagnostics_state,
            parsed_url.as_str(),
            mime_hint,
            control,
        )?;
    if viewer_selection.viewer_id != "viewer:webview" {
        log::debug!(
            "registry viewer '{}' selected for {}; keeping webview path in Phase 0",
            viewer_selection.viewer_id,
            parsed_url.as_str()
        );
    }

    let normalized_url = apply_phase0_protocol_policy(parsed_url, &protocol_resolution);
    Some(Phase0NavigationDecision {
        normalized_url,
        protocol: protocol_resolution,
        viewer: viewer_selection,
    })
}

/// Test-harness helper: evaluate workspace access control for an inbound sync.
///
/// Returns `true` if the peer is permitted to apply the described sync (access granted),
/// or `false` if the sync should be rejected.  When rejected the
/// `verse.sync.access_denied` diagnostic channel is emitted through `diagnostics_state`.
/// No graph mutations are performed by this function.
#[cfg(test)]
pub(crate) fn phase5_check_verse_workspace_sync_access_for_tests(
    diagnostics_state: &crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    peers: &[crate::mods::native::verse::TrustedPeer],
    peer_id: iroh::NodeId,
    workspace_id: &str,
    has_mutating_intents: bool,
) -> bool {
    use crate::mods::native::verse::AccessLevel;
    use crate::mods::native::verse::sync_worker::resolve_peer_grant;

    let access = resolve_peer_grant(peers, peer_id, workspace_id);

    let Some(access) = access else {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_VERSE_SYNC_ACCESS_DENIED, 0);
        return false;
    };

    if access == AccessLevel::ReadOnly && has_mutating_intents {
        diagnostics_state.emit_message_received_for_tests(CHANNEL_VERSE_SYNC_ACCESS_DENIED, 0);
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_registry_resolves_known_scheme_and_falls_back_for_unknown() {
        let registry = ProtocolRegistry::default();
        let https = registry.resolve("https://example.com");
        assert!(https.supported);
        assert!(!https.fallback_used);

        let custom = registry.resolve("foo://example.com");
        assert!(!custom.supported);
        assert!(custom.fallback_used);
        assert_eq!(custom.matched_scheme, "https");

        let graphshell = registry.resolve("graphshell://settings");
        assert!(graphshell.supported);
        assert_eq!(graphshell.matched_scheme, "graphshell");
        assert_eq!(
            graphshell.inferred_mime_hint.as_deref(),
            Some("application/x-graphshell-settings")
        );
    }

    #[test]
    fn viewer_registry_prefers_mime_then_extension_then_fallback() {
        let registry = ViewerRegistry::default();
        let by_mime = registry.select_for_uri("https://example.com/file.bin", Some("text/csv"));
        assert_eq!(by_mime.viewer_id, "viewer:csv");
        assert_eq!(by_mime.matched_by, "mime");

        let by_ext = registry.select_for_uri("https://example.com/readme.md", None);
        assert_eq!(by_ext.viewer_id, "viewer:markdown");
        assert_eq!(by_ext.matched_by, "extension");

        let fallback = registry.select_for_uri("https://example.com/archive.unknown", None);
        assert!(fallback.fallback_used);
        assert_eq!(fallback.viewer_id, "viewer:webview");

        let internal = registry.select_for_uri("graphshell://settings/history", None);
        assert_eq!(internal.viewer_id, "viewer:settings");
        assert_eq!(internal.matched_by, "internal");
    }

    #[test]
    fn new_with_mods_applies_provider_wiring_to_runtime_viewer_dispatch() {
        let baseline = ViewerRegistry::default()
            .select_for_uri("https://example.com/diagram.svg", Some("image/svg+xml"));
        let runtime = RegistryRuntime::new_with_mods();
        let (_, viewer) = runtime
            .observe_navigation_url_with_control(
                "https://example.com/diagram.svg",
                Some("image/svg+xml"),
                ProtocolResolveControl::default(),
            )
            .expect("default protocol resolve control should be active");

        assert!(baseline.fallback_used);
        assert_eq!(viewer.viewer_id, "viewer:webview");
        assert!(!viewer.fallback_used);
        assert_ne!(viewer.matched_by, "fallback");
    }

    #[test]
    fn provider_wired_runtime_applies_protocol_provider_dispatch() {
        let baseline = ProtocolRegistry::default().resolve("modtest://example.com/path");
        assert!(!baseline.supported);
        assert!(baseline.fallback_used);

        let mut protocol_providers = ProtocolHandlerProviders::new();
        protocol_providers.register_fn(|registry| {
            registry.register_scheme("modtest", "protocol:modtest");
        });

        let runtime = RegistryRuntime::new_with_provider_registries_for_tests(
            protocol_providers,
            ViewerHandlerProviders::new(),
        );

        let (protocol, _viewer) = runtime
            .observe_navigation_url_with_control(
                "modtest://example.com/path",
                None,
                ProtocolResolveControl::default(),
            )
            .expect("default protocol resolve control should be active");

        assert!(protocol.supported);
        assert!(!protocol.fallback_used);
        assert_eq!(protocol.matched_scheme, "modtest");
    }

    #[test]
    fn phase0_dispatch_uses_provider_wired_runtime_singleton() {
        let baseline = ViewerRegistry::default()
            .select_for_uri("https://example.com/diagram.svg", Some("image/svg+xml"));
        let parsed = ServoUrl::parse("https://example.com/diagram.svg").expect("url should parse");

        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("image/svg+xml"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should be active");

        assert!(baseline.fallback_used);
        assert_eq!(decision.viewer.viewer_id, "viewer:webview");
        assert!(!decision.viewer.fallback_used);
        assert_ne!(decision.viewer.matched_by, "fallback");
    }

    #[test]
    fn diagnostics_registry_declares_phase0_channels_with_versions() {
        let channels = diagnostics::phase0_required_channels();
        assert!(channels.len() >= 7);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_PROTOCOL_RESOLVE_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VIEWER_SELECT_SUCCEEDED)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase2_action_channels_with_versions() {
        let channels = diagnostics::phase2_required_channels();
        assert!(channels.len() >= 3);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_SUCCEEDED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_ACTION_EXECUTE_FAILED)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase2_input_channels_with_versions() {
        let channels = diagnostics::phase2_required_channels();
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_RESOLVED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_MISSING)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_INPUT_BINDING_CONFLICT)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase3_identity_channels_with_versions() {
        let channels = diagnostics::phase3_required_channels();
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_STARTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_SUCCEEDED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_SIGN_FAILED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_IDENTITY_KEY_UNAVAILABLE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_GL_STATE_VIOLATION)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE)
        );
    }

    #[test]
    fn diagnostics_registry_declares_phase5_verse_channels_with_versions() {
        let channels = diagnostics::phase5_required_channels();
        assert!(channels.len() >= 6);
        assert!(channels.iter().all(|entry| entry.schema_version > 0));
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_UNIT_SENT)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_UNIT_RECEIVED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_INTENT_APPLIED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_ACCESS_DENIED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_CONNECTION_REJECTED)
        );
        assert!(
            channels
                .iter()
                .any(|entry| entry.channel_id == CHANNEL_VERSE_SYNC_IDENTITY_GENERATED)
        );
    }

    #[test]
    fn diagnostics_registry_exposes_phase5_invariants_via_api_surface() {
        let expected_ids = diagnostics::phase5_required_invariant_ids();
        assert_eq!(expected_ids.len(), 2);

        let invariants = diagnostics::list_invariants_snapshot();
        for invariant_id in expected_ids {
            assert!(
                invariants
                    .iter()
                    .any(|entry| entry.invariant_id == *invariant_id),
                "missing required phase5 invariant: {invariant_id}"
            );
        }
    }

    #[test]
    fn phase3_identity_registry_signs_default_payload() {
        let signature = phase3_sign_identity_payload("identity:default", b"payload");
        assert!(
            signature
                .as_deref()
                .is_some_and(|sig| sig.starts_with("sig:"))
        );
    }

    #[test]
    fn phase2_input_registry_resolves_toolbar_submit_binding() {
        assert!(phase2_resolve_toolbar_submit_binding());
    }

    #[test]
    fn phase2_input_registry_resolves_toolbar_nav_reload_binding() {
        assert!(phase2_resolve_input_binding(
            crate::shell::desktop::runtime::registries::input::INPUT_BINDING_TOOLBAR_NAV_RELOAD,
        ));
    }

    #[test]
    fn phase2_lens_registry_resolves_default_lens_id() {
        let lens =
            phase2_resolve_lens(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT);
        assert_eq!(lens.name, "Default");
        assert_eq!(
            lens.lens_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT)
        );
        assert_eq!(
            lens.physics_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::physics::PHYSICS_ID_DEFAULT)
        );
        assert_eq!(
            lens.layout_id.as_deref(),
            Some(crate::registries::atomic::layout::LAYOUT_ID_DEFAULT)
        );
        assert_eq!(
            lens.theme_id.as_deref(),
            Some(crate::registries::atomic::theme::THEME_ID_DEFAULT)
        );
    }

    #[test]
    fn phase2_lens_registry_falls_back_for_unknown_lens_id() {
        let lens = phase2_resolve_lens("lens:unknown");
        assert_eq!(lens.name, "Default");
        assert_eq!(
            lens.lens_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::lens::LENS_ID_DEFAULT)
        );
    }

    #[test]
    fn phase2_lens_component_resolution_normalizes_unknown_component_ids() {
        let mut lens = crate::app::LensConfig::default();
        lens.physics_id = Some("physics:unknown".to_string());
        lens.layout_id = Some("layout:unknown".to_string());
        lens.theme_id = Some("theme:unknown".to_string());

        let normalized = phase2_resolve_lens_components(&lens);

        assert_eq!(
            normalized.physics_id.as_deref(),
            Some(crate::shell::desktop::runtime::registries::physics::PHYSICS_ID_DEFAULT)
        );
        assert_eq!(
            normalized.layout_id.as_deref(),
            Some(crate::registries::atomic::layout::LAYOUT_ID_DEFAULT)
        );
        assert_eq!(
            normalized.theme_id.as_deref(),
            Some(crate::registries::atomic::theme::THEME_ID_DEFAULT)
        );
    }

    #[test]
    fn phase2_action_registry_omnibox_search_selects_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.graph.add_node(
            "https://example.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        if let Some(node) = app.workspace.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }

        let intents = phase2_execute_omnibox_node_search_action(&app, "example handle");
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SelectNode { key: selected, .. }) if *selected == key
        ));
    }

    #[test]
    fn phase2_action_registry_graph_submit_updates_selected_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.graph.add_node(
            "https://start.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.workspace.selected_nodes.select(key, false);

        let (open_selected_tile, intents) =
            phase2_execute_graph_view_submit_action(&app, "https://next.com");
        assert!(open_selected_tile);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://next.com"
        ));
    }

    #[test]
    fn phase2_action_registry_detail_submit_updates_focused_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.graph.add_node(
            "https://start.com".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let (open_selected_tile, intents) =
            phase2_execute_detail_view_submit_action(&app, "https://detail-next.com", Some(key));

        assert!(!open_selected_tile);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://detail-next.com"
        ));
    }

    #[test]
    fn phase5_action_registry_sync_now_emits_sync_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let intents = phase5_execute_verse_sync_now_action(&app);

        assert_eq!(intents.len(), 1);
        assert!(matches!(intents.first(), Some(GraphIntent::SyncNow)));
    }

    #[test]
    fn phase5_action_registry_forget_device_emits_peer_targeted_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();

        let intents = phase5_execute_verse_forget_device_action(&app, &peer_id);

        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::ForgetDevice { peer_id: emitted }) if emitted == &peer_id
        ));
    }

    #[test]
    fn phase5_action_registry_pair_local_peer_returns_no_reducer_intents() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();

        let intents = phase5_execute_verse_pair_local_peer_action(&app, &peer_id);
        assert!(intents.is_empty());
    }

    #[test]
    fn phase5_action_registry_share_workspace_returns_no_reducer_intents() {
        let app = GraphBrowserApp::new_for_testing();
        let intents = phase5_execute_verse_share_workspace_action(&app, "workspace:test");
        assert!(intents.is_empty());
    }

    #[test]
    fn phase0_normalization_rewrites_unknown_scheme_to_protocol_fallback() {
        let parsed = ServoUrl::parse("foo://example.com/path").expect("url should parse");
        let rewritten =
            phase0_decide_navigation_with_control(parsed, None, ProtocolResolveControl::default())
                .expect("default protocol resolve control should not cancel")
                .normalized_url;

        assert_eq!(rewritten.scheme(), "https");
        assert_eq!(rewritten.host_str(), Some("example.com"));
    }

    #[test]
    fn phase0_decision_prefers_mime_hint_for_viewer_selection() {
        let parsed = ServoUrl::parse("https://example.com/file.bin").expect("url should parse");
        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("text/csv"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(decision.viewer.viewer_id, "viewer:csv");
        assert_eq!(decision.viewer.matched_by, "mime");
        assert_eq!(decision.normalized_url.scheme(), "https");
    }

    #[test]
    fn phase0_decision_uses_protocol_inferred_mime_hint_when_explicit_hint_missing() {
        let parsed =
            ServoUrl::parse("https://example.com/download/no_extension").expect("url should parse");
        let decision =
            phase0_decide_navigation_with_control(parsed, None, ProtocolResolveControl::default())
                .expect("default protocol resolve control should not cancel");

        assert_eq!(decision.protocol.inferred_mime_hint.as_deref(), None);
        assert_eq!(decision.viewer.viewer_id, "viewer:webview");

        let data_uri = ServoUrl::parse("data:text/csv,foo,bar").expect("data URI should parse");
        let data_decision = phase0_decide_navigation_with_control(
            data_uri,
            None,
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(
            data_decision.protocol.inferred_mime_hint.as_deref(),
            Some("text/csv")
        );
        assert_eq!(data_decision.viewer.viewer_id, "viewer:csv");
        assert_eq!(data_decision.viewer.matched_by, "mime");
    }

    #[test]
    fn phase0_decision_prefers_explicit_mime_hint_over_protocol_inferred_hint() {
        let parsed = ServoUrl::parse("data:text/csv,foo,bar").expect("data URI should parse");
        let decision = phase0_decide_navigation_with_control(
            parsed,
            Some("application/pdf"),
            ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel");

        assert_eq!(
            decision.protocol.inferred_mime_hint.as_deref(),
            Some("text/csv")
        );
        assert_eq!(decision.viewer.matched_by, "mime");
        assert_ne!(decision.viewer.viewer_id, "viewer:csv");
    }

    #[test]
    fn phase0_decision_returns_none_when_protocol_resolution_is_cancelled() {
        let parsed = ServoUrl::parse("https://example.com/readme.md").expect("url should parse");

        let decision = phase0_decide_navigation_with_control(
            parsed,
            None,
            ProtocolResolveControl::cancelled(),
        );

        assert!(decision.is_none());
    }

    #[test]
    fn phase3_viewer_surface_resolution_returns_default_profile() {
        let resolution = phase3_resolve_viewer_surface_profile("viewer:webview");
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DEFAULT);
    }

    #[test]
    fn diagnostics_registry_accepts_namespaced_mod_channel_registration() {
        let result = register_mod_diagnostics_channel(
            "planner",
            "mod.planner.agent.think",
            1,
            Some("planner thought loop".to_string()),
        )
        .expect("mod channel registration should succeed");

        assert!(result);
    }

    #[test]
    fn diagnostics_registry_rejects_non_namespaced_verse_channel_registration() {
        let result = register_verse_diagnostics_channel(
            "peer-a",
            "agent.decide",
            1,
            Some("invalid namespace".to_string()),
        );

        assert!(matches!(
            result,
            Err(diagnostics::ChannelRegistrationError::InvalidOwnership { .. })
        ));
    }

    #[test]
    fn diagnostics_registry_registers_invariant_extension() {
        let created = register_diagnostics_invariant(diagnostics::DiagnosticsInvariant {
            invariant_id: "invariant.registry.test.layout_timeout".to_string(),
            start_channel: "layout.compute_started".to_string(),
            terminal_channels: vec![
                "layout.compute_succeeded".to_string(),
                "layout.compute_failed".to_string(),
            ],
            timeout_ms: 250,
            owner: diagnostics::DiagnosticsChannelOwner::core(),
            enabled: true,
        })
        .expect("invariant registration should succeed");

        assert!(created);
    }
}
