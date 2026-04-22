/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command palette session state.
//!
//! Per the M4 runtime extraction (§3.2 Command routing), palette session
//! state lives on `GraphshellRuntime` and is host-neutral. Previously the
//! search query and scope were stashed in `egui::Context::data_mut(...)`
//! persistent storage inside [`render::command_palette`], which bound the
//! palette to egui and made the state invisible to the view-model.
//!
//! Session 3 moves these fields onto the runtime, alongside the already-
//! extracted `show_command_palette` / `command_palette_contextual_mode`
//! flags on `graph_app.workspace.chrome_ui`.

/// Scope filter for command-palette search results.
///
/// Was previously a private enum inside `render::command_palette`; lifted
/// here so it can live on the runtime's `CommandPaletteSession` and flow
/// through `CommandPaletteViewModel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchPaletteScope {
    CurrentTarget,
    ActivePane,
    ActiveGraph,
    Workbench,
}

impl SearchPaletteScope {
    pub const ALL: [Self; 4] = [
        Self::CurrentTarget,
        Self::ActivePane,
        Self::ActiveGraph,
        Self::Workbench,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::CurrentTarget => "Current Target",
            Self::ActivePane => "Active Pane",
            Self::ActiveGraph => "Active Graph",
            Self::Workbench => "Workbench",
        }
    }
}

impl Default for SearchPaletteScope {
    fn default() -> Self {
        Self::Workbench
    }
}

impl std::fmt::Display for SearchPaletteScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CurrentTarget => "current-target",
            Self::ActivePane => "active-pane",
            Self::ActiveGraph => "active-graph",
            Self::Workbench => "workbench",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for SearchPaletteScope {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim() {
            "current-target" => Ok(Self::CurrentTarget),
            "active-pane" => Ok(Self::ActivePane),
            "active-graph" => Ok(Self::ActiveGraph),
            "workbench" => Ok(Self::Workbench),
            _ => Err(()),
        }
    }
}

/// Live session state for the command palette.
///
/// Holds the user's in-progress search query, the chosen scope filter,
/// the keyboard-navigation cursor, and a one-shot "focus the search
/// field next frame" flag. Consumed by the palette widget through the
/// `CommandAuthorityMut` bundle.
#[derive(Clone, Debug, Default)]
pub(crate) struct CommandPaletteSession {
    /// Current search text the user has typed into the palette.
    pub(crate) query: String,
    /// Active scope filter for result filtering.
    pub(crate) scope: SearchPaletteScope,
    /// Keyboard-navigation cursor into the filtered result list.
    /// `None` means no row is highlighted; when the result list is
    /// non-empty this is clamped to `0..filtered.len()` by the widget
    /// before use.
    pub(crate) selected_index: Option<usize>,
    /// Set to `true` when the palette is opened (or re-opened) so the
    /// widget grabs focus for the search field on the next frame. The
    /// widget clears the flag after applying focus.
    pub(crate) focus_search_on_next_frame: bool,
    /// Open-state from the previous frame, so the widget can detect
    /// the closed→open transition and call `open_fresh(...)` exactly
    /// once on open. Always set by the widget before it returns.
    pub(crate) was_open_last_frame: bool,
    /// Last-selected Tier 1 category in contextual palette mode.
    /// Previously stashed in `egui::Context::data_mut(...)` persistent
    /// storage under the `"command_palette_tier1_category"` key; moved
    /// onto the session in M4 session 3's contextual-mode follow-on.
    /// `None` means "use the first available category" (falls back to
    /// `ActionCategory::Graph` when no ordered categories are
    /// available).
    pub(crate) tier1_category: Option<crate::render::action_registry::ActionCategory>,
}

impl CommandPaletteSession {
    /// Reset the session to a newly-opened state: empty query, default
    /// scope, no selection, and the focus flag armed so the search field
    /// grabs keyboard focus on the next frame.
    ///
    /// `default_scope` lets the caller pick the initial scope (a
    /// workspace setting in M4 §h).
    pub(crate) fn open_fresh(&mut self, default_scope: SearchPaletteScope) {
        self.query.clear();
        self.scope = default_scope;
        self.selected_index = None;
        self.focus_search_on_next_frame = true;
    }

    /// Advance the selection cursor by `delta` (positive cycles down,
    /// negative cycles up), wrapping within `result_count`. A no-op
    /// when `result_count` is zero. Initializes to `0` (down) or
    /// `result_count - 1` (up) if no row is currently selected.
    pub(crate) fn step_selection(&mut self, delta: isize, result_count: usize) {
        if result_count == 0 {
            self.selected_index = None;
            return;
        }
        let count = result_count as isize;
        let next = match self.selected_index {
            Some(current) => {
                let current = current as isize;
                ((current + delta).rem_euclid(count)) as usize
            }
            None => {
                if delta >= 0 {
                    0
                } else {
                    (count - 1) as usize
                }
            }
        };
        self.selected_index = Some(next);
    }
}
