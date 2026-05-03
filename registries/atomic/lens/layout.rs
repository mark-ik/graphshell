// Slice 66: body moved to register-lens. Re-export shim at the
// original path. register-lens flattens all sub-module exports to
// its crate root, so we glob-import from there.
pub(crate) use register_lens::LayoutMode;
