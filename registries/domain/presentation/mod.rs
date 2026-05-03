use graphshell_core::color::Color32;

use crate::registries::atomic::lens::{
    PhysicsProfileResolution, THEME_ID_DARK, THEME_ID_LIGHT, ThemeResolution,
    resolve_physics_profile, resolve_theme_data,
};
use crate::registries::domain::layout::profile_registry::ProfileRegistry;

pub(crate) const PRESENTATION_PROFILE_DEFAULT: &str = "presentation:default";
pub(crate) const PRESENTATION_PROFILE_LIGHT: &str = "presentation:light";
pub(crate) const PRESENTATION_PROFILE_DARK: &str = "presentation:dark";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PresentationColor {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    #[serde(default = "PresentationColor::default_alpha")]
    pub(crate) a: u8,
}

impl PresentationColor {
    const fn default_alpha() -> u8 {
        255
    }

    pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub(crate) const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub(crate) fn to_color32(self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }

    pub(crate) fn with_alpha(self, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.r, self.g, self.b, alpha)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PresentationProfile {
    pub(crate) profile_id: String,
    pub(crate) edge_highlight_backdrop: PresentationColor,
    pub(crate) edge_highlight_foreground: PresentationColor,
    pub(crate) lifecycle_active: PresentationColor,
    pub(crate) lifecycle_warm: PresentationColor,
    pub(crate) lifecycle_cold: PresentationColor,
    pub(crate) lifecycle_tombstone: PresentationColor,
    pub(crate) crash_blocked: PresentationColor,
    pub(crate) search_match: PresentationColor,
    pub(crate) search_match_active: PresentationColor,
    pub(crate) hover_target: PresentationColor,
    pub(crate) selection_primary: PresentationColor,
    pub(crate) lasso_stroke: PresentationColor,
    pub(crate) lasso_fill: PresentationColor,
    pub(crate) info_text: PresentationColor,
    pub(crate) controls_text: PresentationColor,
    pub(crate) degraded_receipt_background: PresentationColor,
    pub(crate) degraded_receipt_text: PresentationColor,
    pub(crate) focus_ring: PresentationColor,
    pub(crate) hover_ring: PresentationColor,
}

#[derive(Debug, Clone)]
pub(crate) struct PresentationDomainProfileResolution {
    pub(crate) physics: PhysicsProfileResolution,
    pub(crate) theme: ThemeResolution,
    pub(crate) resolved_profile_id: String,
    pub(crate) matched_profile: bool,
    pub(crate) fallback_profile_used: bool,
    pub(crate) profile: PresentationProfile,
}

pub(crate) struct PresentationDomainRegistry {
    profiles: ProfileRegistry<PresentationProfile>,
}

impl Default for PresentationDomainRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: ProfileRegistry::new(PRESENTATION_PROFILE_DEFAULT),
        };
        registry.register(PRESENTATION_PROFILE_DEFAULT, default_profile());
        registry.register(PRESENTATION_PROFILE_LIGHT, light_profile());
        registry.register(PRESENTATION_PROFILE_DARK, dark_profile());
        registry
    }
}

impl PresentationDomainRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: PresentationProfile) {
        self.profiles.register(profile_id, profile);
    }

    pub(crate) fn resolve_profile(
        &self,
        physics_id: &str,
        theme_id: &str,
    ) -> PresentationDomainProfileResolution {
        let theme = resolve_theme_data(theme_id);
        let profile_resolution = self.profiles.resolve(
            profile_id_for_theme(theme.resolved_id.as_str()),
            "presentation profile",
        );
        let physics = resolve_physics_profile(physics_id);
        let fallback_profile_used =
            profile_resolution.fallback_used || physics.fallback_used || theme.fallback_used;
        PresentationDomainProfileResolution {
            physics,
            theme,
            resolved_profile_id: profile_resolution.resolved_id,
            matched_profile: profile_resolution.matched,
            fallback_profile_used,
            profile: profile_resolution.profile,
        }
    }
}

fn profile_id_for_theme(theme_id: &str) -> &'static str {
    match theme_id {
        THEME_ID_LIGHT => PRESENTATION_PROFILE_LIGHT,
        THEME_ID_DARK => PRESENTATION_PROFILE_DARK,
        _ => PRESENTATION_PROFILE_DEFAULT,
    }
}

fn default_profile() -> PresentationProfile {
    PresentationProfile {
        profile_id: PRESENTATION_PROFILE_DEFAULT.to_string(),
        edge_highlight_backdrop: PresentationColor::rgba(10, 30, 40, 120),
        edge_highlight_foreground: PresentationColor::rgb(80, 220, 255),
        lifecycle_active: PresentationColor::rgb(100, 200, 255),
        lifecycle_warm: PresentationColor::rgb(120, 170, 205),
        lifecycle_cold: PresentationColor::rgb(140, 140, 165),
        lifecycle_tombstone: PresentationColor::rgb(96, 96, 96),
        crash_blocked: PresentationColor::rgb(205, 112, 82),
        search_match: PresentationColor::rgb(95, 220, 130),
        search_match_active: PresentationColor::rgb(140, 255, 140),
        hover_target: PresentationColor::rgb(255, 150, 80),
        selection_primary: PresentationColor::rgb(255, 200, 100),
        lasso_stroke: PresentationColor::rgb(90, 220, 170),
        lasso_fill: PresentationColor::rgba(90, 220, 170, 28),
        info_text: PresentationColor::rgb(200, 200, 200),
        controls_text: PresentationColor::rgb(150, 150, 150),
        degraded_receipt_background: PresentationColor::rgba(45, 30, 20, 225),
        degraded_receipt_text: PresentationColor::rgb(255, 210, 120),
        focus_ring: PresentationColor::rgb(120, 200, 255),
        hover_ring: PresentationColor::rgba(180, 180, 190, 180),
    }
}

fn dark_profile() -> PresentationProfile {
    PresentationProfile {
        profile_id: PRESENTATION_PROFILE_DARK.to_string(),
        edge_highlight_backdrop: PresentationColor::rgba(6, 18, 28, 150),
        edge_highlight_foreground: PresentationColor::rgb(110, 170, 255),
        lifecycle_active: PresentationColor::rgb(120, 185, 255),
        lifecycle_warm: PresentationColor::rgb(122, 154, 212),
        lifecycle_cold: PresentationColor::rgb(132, 132, 176),
        lifecycle_tombstone: PresentationColor::rgb(82, 82, 96),
        crash_blocked: PresentationColor::rgb(214, 120, 92),
        search_match: PresentationColor::rgb(112, 214, 158),
        search_match_active: PresentationColor::rgb(162, 245, 188),
        hover_target: PresentationColor::rgb(255, 166, 104),
        selection_primary: PresentationColor::rgb(255, 214, 134),
        lasso_stroke: PresentationColor::rgb(110, 205, 182),
        lasso_fill: PresentationColor::rgba(110, 205, 182, 34),
        info_text: PresentationColor::rgb(214, 214, 222),
        controls_text: PresentationColor::rgb(164, 164, 176),
        degraded_receipt_background: PresentationColor::rgba(32, 22, 18, 235),
        degraded_receipt_text: PresentationColor::rgb(255, 214, 142),
        focus_ring: PresentationColor::rgb(140, 182, 255),
        hover_ring: PresentationColor::rgba(170, 176, 194, 190),
    }
}

fn light_profile() -> PresentationProfile {
    PresentationProfile {
        profile_id: PRESENTATION_PROFILE_LIGHT.to_string(),
        edge_highlight_backdrop: PresentationColor::rgba(208, 224, 246, 170),
        edge_highlight_foreground: PresentationColor::rgb(54, 120, 212),
        lifecycle_active: PresentationColor::rgb(54, 120, 212),
        lifecycle_warm: PresentationColor::rgb(94, 136, 184),
        lifecycle_cold: PresentationColor::rgb(130, 136, 148),
        lifecycle_tombstone: PresentationColor::rgb(154, 156, 164),
        crash_blocked: PresentationColor::rgb(184, 92, 60),
        search_match: PresentationColor::rgb(50, 170, 94),
        search_match_active: PresentationColor::rgb(38, 146, 80),
        hover_target: PresentationColor::rgb(214, 120, 52),
        selection_primary: PresentationColor::rgb(214, 160, 56),
        lasso_stroke: PresentationColor::rgb(50, 176, 150),
        lasso_fill: PresentationColor::rgba(50, 176, 150, 28),
        info_text: PresentationColor::rgb(76, 82, 92),
        controls_text: PresentationColor::rgb(108, 112, 120),
        degraded_receipt_background: PresentationColor::rgba(246, 238, 224, 232),
        degraded_receipt_text: PresentationColor::rgb(138, 90, 18),
        focus_ring: PresentationColor::rgb(54, 120, 212),
        hover_ring: PresentationColor::rgba(152, 160, 172, 164),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::lens::PHYSICS_ID_SCATTER;

    #[test]
    fn presentation_domain_resolves_default_profile() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile(
            crate::registries::atomic::lens::PHYSICS_ID_DEFAULT,
            crate::registries::atomic::lens::THEME_ID_DEFAULT,
        );

        assert!(resolution.physics.matched);
        assert!(resolution.theme.matched);
        assert!(!resolution.physics.fallback_used);
        assert!(!resolution.theme.fallback_used);
        assert_eq!(resolution.resolved_profile_id, PRESENTATION_PROFILE_DEFAULT);
        assert_eq!(
            resolution.profile.edge_highlight_foreground,
            PresentationColor::rgb(80, 220, 255)
        );
    }

    #[test]
    fn presentation_domain_falls_back_independently() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile("physics:unknown", "theme:unknown");

        assert!(resolution.physics.fallback_used);
        assert!(resolution.theme.fallback_used);
        assert!(resolution.fallback_profile_used);
        assert_eq!(resolution.resolved_profile_id, PRESENTATION_PROFILE_DEFAULT);
    }

    #[test]
    fn presentation_domain_uses_dark_profile_for_dark_theme() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile(PHYSICS_ID_SCATTER, THEME_ID_DARK);

        assert_eq!(resolution.theme.resolved_id, THEME_ID_DARK);
        assert_eq!(resolution.physics.resolved_id, PHYSICS_ID_SCATTER);
        assert_eq!(resolution.resolved_profile_id, PRESENTATION_PROFILE_DARK);
        assert_eq!(
            resolution.profile.focus_ring,
            PresentationColor::rgb(140, 182, 255)
        );
    }

    #[test]
    fn presentation_domain_uses_light_profile_for_light_theme() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile(
            crate::registries::atomic::lens::PHYSICS_ID_DEFAULT,
            THEME_ID_LIGHT,
        );

        assert_eq!(resolution.theme.resolved_id, THEME_ID_LIGHT);
        assert_eq!(resolution.resolved_profile_id, PRESENTATION_PROFILE_LIGHT);
        assert_eq!(
            resolution.profile.edge_highlight_foreground,
            PresentationColor::rgb(54, 120, 212)
        );
    }
}
