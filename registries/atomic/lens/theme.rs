use serde::Deserialize;

pub(crate) const THEME_ID_DEFAULT: &str = "theme:default";
pub(crate) const THEME_ID_LIGHT: &str = "theme:light";
pub(crate) const THEME_ID_DARK: &str = "theme:dark";
pub(crate) const THEME_ID_HIGH_CONTRAST: &str = "theme:high_contrast";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThemeData {
    pub background_rgb: (u8, u8, u8),
    pub accent_rgb: (u8, u8, u8),
    pub font_scale: f32,
    pub stroke_width: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) theme_id: String,
    pub(crate) theme: ThemeData,
}

pub(crate) fn resolve_theme_data(theme_id: &str) -> ThemeResolution {
    let requested = theme_id.trim().to_ascii_lowercase();
    let fallback_theme = default_theme();

    if requested.is_empty() {
        return ThemeResolution {
            requested_id: requested,
            resolved_id: THEME_ID_DEFAULT.to_string(),
            matched: false,
            fallback_used: true,
            theme_id: THEME_ID_DEFAULT.to_string(),
            theme: fallback_theme,
        };
    }

    let theme = match requested.as_str() {
        THEME_ID_DEFAULT => Some(default_theme()),
        THEME_ID_LIGHT => Some(default_theme()),
        THEME_ID_DARK => Some(dark_theme()),
        THEME_ID_HIGH_CONTRAST => Some(high_contrast_theme()),
        _ => None,
    };

    if let Some(theme) = theme {
        return ThemeResolution {
            requested_id: requested.clone(),
            resolved_id: requested.clone(),
            matched: true,
            fallback_used: false,
            theme_id: requested,
            theme,
        };
    }

    ThemeResolution {
        requested_id: requested,
        resolved_id: THEME_ID_DEFAULT.to_string(),
        matched: false,
        fallback_used: true,
        theme_id: THEME_ID_DEFAULT.to_string(),
        theme: fallback_theme,
    }
}

pub(crate) fn theme_data_id(theme: &ThemeData) -> &'static str {
    if *theme == dark_theme() {
        THEME_ID_DARK
    } else {
        THEME_ID_DEFAULT
    }
}

fn default_theme() -> ThemeData {
    ThemeData {
        background_rgb: (20, 20, 25),
        accent_rgb: (80, 220, 255),
        font_scale: 1.0,
        stroke_width: 1.0,
    }
}

fn dark_theme() -> ThemeData {
    ThemeData {
        background_rgb: (14, 14, 18),
        accent_rgb: (110, 170, 255),
        font_scale: 1.0,
        stroke_width: 1.0,
    }
}

fn high_contrast_theme() -> ThemeData {
    ThemeData {
        background_rgb: (0, 0, 0),
        accent_rgb: (255, 230, 0),
        font_scale: 1.1,
        stroke_width: 1.5,
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PersistedThemeValue {
    Theme(ThemeData),
    ThemeId(String),
}

pub(crate) fn deserialize_optional_theme_data<'de, D>(
    deserializer: D,
) -> Result<Option<ThemeData>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let persisted = Option::<PersistedThemeValue>::deserialize(deserializer)?;
    Ok(persisted.map(|value| match value {
        PersistedThemeValue::Theme(theme) => theme,
        PersistedThemeValue::ThemeId(theme_id) => resolve_theme_data(&theme_id).theme,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_lookup_falls_back_for_unknown_id() {
        let resolution = resolve_theme_data("theme:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme.background_rgb, (20, 20, 25));
    }
}
