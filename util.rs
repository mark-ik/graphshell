/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 56: Verso URI scheme parsing (VersoAddress, VersoViewTarget,
// GraphshellSettingsPath, GraphAddress, NodeAddress, NoteAddress,
// truncate_with_ellipsis, scheme constants) moved to
// `graphshell_core::verso_address` per the workspace architecture
// proposal. They were pure URI-string parsing that didn't belong
// inside the binary root next to the egui-flavoured CoordBridge.
//
// Re-exported here so existing call sites importing via
// `crate::util::*` continue to work. CoordBridge stays in tree
// because it's egui-specific (the egui-host build is frozen per
// iced jump-ship plan §S1).

#[cfg(feature = "egui-host")]
use egui::Pos2;
use euclid::Point2D;

pub(crate) use graphshell_core::verso_address::{
    GRAPHSHELL_SCHEME_PREFIX, GRAPH_SCHEME_PREFIX, GraphAddress, GraphshellSettingsPath,
    NODE_SCHEME_PREFIX, NOTES_SCHEME_PREFIX, NodeAddress, NoteAddress, VERSO_SCHEME_PREFIX,
    VersoAddress, VersoViewTarget, truncate_with_ellipsis,
};

pub(crate) trait CoordBridge {
    #[cfg(feature = "egui-host")]
    fn to_pos2(self) -> Pos2;
    fn to_point2d<U>(self) -> Point2D<f32, U>;
}

impl<U> CoordBridge for Point2D<f32, U> {
    #[cfg(feature = "egui-host")]
    fn to_pos2(self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }

    fn to_point2d<V>(self) -> Point2D<f32, V> {
        Point2D::new(self.x, self.y)
    }
}

#[cfg(feature = "egui-host")]
impl CoordBridge for Pos2 {
    fn to_pos2(self) -> Pos2 {
        self
    }

    fn to_point2d<U>(self) -> Point2D<f32, U> {
        Point2D::new(self.x, self.y)
    }
}
