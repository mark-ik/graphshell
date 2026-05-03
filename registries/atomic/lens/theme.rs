// Slice 66: body moved to register-lens. Re-export shim.
pub(crate) use register_lens::{
    deserialize_optional_theme_data, resolve_theme_data, theme_data_id, ThemeData,
    ThemeResolution, THEME_ID_DARK, THEME_ID_DEFAULT, THEME_ID_LIGHT,
};
