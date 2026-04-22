/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable host-facing mutation bundles ("authorities").
//!
//! The M4 runtime extraction bundles per-subsystem mutable references
//! into `*AuthorityMut<'a>` structs so phase-pipeline functions can
//! accept one handle per subsystem instead of threading each field
//! individually. Two of the four bundles are now fully portable:
//!
//! - [`GraphSearchAuthorityMut`] — five `&mut T` refs where every `T`
//!   is portable (`bool`, `String`, `Vec<NodeKey>`, `Option<usize>`).
//! - [`CommandAuthorityMut`] — toggle flag + a reference to the
//!   portable [`CommandPaletteSession`].
//!
//! The other two authority bundles remain shell-side:
//!
//! - `FocusAuthorityMut` — blocked on `std::time::Instant` in
//!   `focus_ring_started_at` (not available on
//!   `wasm32-unknown-unknown`). Moves with slice 8's time-portability
//!   decision.
//! - `ToolbarAuthorityMut` — blocked on `OmnibarSearchSession` (still
//!   carries `Instant`), `ProviderSuggestionDriver` (holds
//!   `crossbeam_channel::Receiver<T>`), and `&CommandSurfaceTelemetry`
//!   (still behind `Mutex`). Moves with slices 5b / 6-follow-on.
//!
//! Pre-M4 slice 9 (2026-04-22) both portable bundles lived in
//! `shell/desktop/ui/gui_state.rs`. The graphshell crate re-exports
//! them from their original locations so existing import paths still
//! resolve.

use crate::graph::NodeKey;
use crate::shell_state::command_palette::{CommandPaletteSession, SearchPaletteScope};

/// Host-facing mutation handle for graph-search session state.
///
/// Bundles the five `graph_search_*` fields on `GraphshellRuntime`
/// (`open`, `query`, `filter_mode`, `matches`, `active_match_index`)
/// that previously flowed through `ExecuteUpdateFrameArgs` as
/// individual `&mut` parameters. Callers use [`close`] / [`set_open`] /
/// [`set_query`] instead of field pokes.
///
/// [`close`]: Self::close
/// [`set_open`]: Self::set_open
/// [`set_query`]: Self::set_query
pub struct GraphSearchAuthorityMut<'a> {
    pub open: &'a mut bool,
    pub query: &'a mut String,
    pub filter_mode: &'a mut bool,
    pub matches: &'a mut Vec<NodeKey>,
    pub active_match_index: &'a mut Option<usize>,
}

impl<'a> GraphSearchAuthorityMut<'a> {
    /// Reborrow the bundle with a shorter lifetime. Needed when the
    /// bundle is threaded through nested phase calls without being
    /// moved out of the outer scope.
    pub fn reborrow(&mut self) -> GraphSearchAuthorityMut<'_> {
        GraphSearchAuthorityMut {
            open: &mut *self.open,
            query: &mut *self.query,
            filter_mode: &mut *self.filter_mode,
            matches: &mut *self.matches,
            active_match_index: &mut *self.active_match_index,
        }
    }

    pub fn is_open(&self) -> bool {
        *self.open
    }

    pub fn set_open(&mut self, value: bool) {
        *self.open = value;
    }

    /// Close the graph-search panel and reset its transient state. The
    /// query text is preserved so a subsequent `set_open(true)` restores
    /// it.
    pub fn close(&mut self) {
        *self.open = false;
        self.matches.clear();
        *self.active_match_index = None;
    }

    pub fn query(&self) -> &str {
        self.query.as_str()
    }

    pub fn query_mut(&mut self) -> &mut String {
        &mut *self.query
    }

    pub fn set_query(&mut self, value: impl Into<String>) {
        *self.query = value.into();
    }

    pub fn filter_mode_active(&self) -> bool {
        *self.filter_mode
    }

    pub fn set_filter_mode(&mut self, value: bool) {
        *self.filter_mode = value;
    }

    pub fn toggle_filter_mode(&mut self) {
        *self.filter_mode = !*self.filter_mode;
    }

    pub fn matches(&self) -> &[NodeKey] {
        self.matches.as_slice()
    }

    pub fn set_matches(&mut self, matches: Vec<NodeKey>) {
        *self.matches = matches;
    }

    pub fn active_match_index(&self) -> Option<usize> {
        *self.active_match_index
    }

    pub fn set_active_match_index(&mut self, value: Option<usize>) {
        *self.active_match_index = value;
    }
}

/// Host-facing mutation handle for runtime-owned command-palette state.
///
/// Per the M4 runtime extraction (§3.2 Command routing), the palette's
/// toggle request and its session state (search query, scope filter,
/// selection cursor, focus-on-open flag) live on `GraphshellRuntime`.
/// The widget mutates them through this bundle instead of stashing
/// search state in `egui::Context::data_mut(...)` persistent storage,
/// the pre-M4 pattern that did not survive a host migration.
///
/// The palette's **open flag** (`show_command_palette`) deliberately
/// stays on `graph_app.workspace.chrome_ui`, not on the runtime, and
/// is NOT a member of this bundle. It's one of eight mutually-
/// exclusive modal flags that `app::ux_navigation` manages as a
/// coordinated cluster; lifting any one flag in isolation would split
/// the cluster. Future work: a "modal surface extraction" session
/// lifts the whole cluster to the runtime as a coherent bundle; until
/// then the widget and callers continue to read/write the open flag
/// directly through `graph_app`.
pub struct CommandAuthorityMut<'a> {
    pub toggle_requested: &'a mut bool,
    pub session: &'a mut CommandPaletteSession,
}

impl<'a> CommandAuthorityMut<'a> {
    pub fn reborrow(&mut self) -> CommandAuthorityMut<'_> {
        CommandAuthorityMut {
            toggle_requested: &mut *self.toggle_requested,
            session: &mut *self.session,
        }
    }

    pub fn toggle_requested(&self) -> bool {
        *self.toggle_requested
    }

    pub fn clear_toggle_request(&mut self) {
        *self.toggle_requested = false;
    }

    /// Arm the session for a fresh open: reset query/scope/selection
    /// and request keyboard focus for the search field on the next
    /// frame. The palette's visibility flag (`show_command_palette`)
    /// lives on workspace state and is flipped by the caller; this
    /// method only touches runtime-owned session state.
    pub fn prime_fresh_open(&mut self, default_scope: SearchPaletteScope) {
        self.session.open_fresh(default_scope);
    }

    pub fn session(&self) -> &CommandPaletteSession {
        self.session
    }

    pub fn session_mut(&mut self) -> &mut CommandPaletteSession {
        self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_search_close_clears_matches_but_preserves_query() {
        // The session's query-preservation contract is load-bearing for
        // the toggle UX: users can dismiss the overlay, navigate, and
        // reopen to see the same text. Pin it.
        let mut open = true;
        let mut query = "rust".to_string();
        let mut filter_mode = true;
        let mut matches = vec![NodeKey::new(1), NodeKey::new(2)];
        let mut active_match_index = Some(0);

        let mut authority = GraphSearchAuthorityMut {
            open: &mut open,
            query: &mut query,
            filter_mode: &mut filter_mode,
            matches: &mut matches,
            active_match_index: &mut active_match_index,
        };

        authority.close();

        assert!(!*authority.open);
        assert_eq!(authority.query(), "rust");
        // filter_mode is intentionally NOT reset by close — it's a
        // user preference that persists across open/close cycles.
        assert!(*authority.filter_mode);
        assert!(authority.matches().is_empty());
        assert_eq!(authority.active_match_index(), None);
    }

    #[test]
    fn graph_search_reborrow_preserves_access_after_nested_mutation() {
        let mut open = false;
        let mut query = String::new();
        let mut filter_mode = false;
        let mut matches = Vec::new();
        let mut active_match_index = None;

        let mut outer = GraphSearchAuthorityMut {
            open: &mut open,
            query: &mut query,
            filter_mode: &mut filter_mode,
            matches: &mut matches,
            active_match_index: &mut active_match_index,
        };

        {
            let mut inner = outer.reborrow();
            inner.set_query("async");
            inner.set_open(true);
        }

        assert_eq!(outer.query(), "async");
        assert!(outer.is_open());
    }

    #[test]
    fn graph_search_toggle_filter_mode_flips_independently_of_open() {
        let mut open = false;
        let mut query = String::new();
        let mut filter_mode = false;
        let mut matches = Vec::new();
        let mut active_match_index = None;

        let mut authority = GraphSearchAuthorityMut {
            open: &mut open,
            query: &mut query,
            filter_mode: &mut filter_mode,
            matches: &mut matches,
            active_match_index: &mut active_match_index,
        };

        authority.toggle_filter_mode();
        assert!(authority.filter_mode_active());
        authority.toggle_filter_mode();
        assert!(!authority.filter_mode_active());
    }

    #[test]
    fn command_authority_prime_fresh_open_resets_session_state() {
        let mut toggle = false;
        let mut session = CommandPaletteSession::default();
        session.query = "stale".into();
        session.selected_index = Some(3);

        let mut authority = CommandAuthorityMut {
            toggle_requested: &mut toggle,
            session: &mut session,
        };

        authority.prime_fresh_open(SearchPaletteScope::ActivePane);

        assert_eq!(authority.session().scope, SearchPaletteScope::ActivePane);
        assert!(authority.session().query.is_empty());
        assert_eq!(authority.session().selected_index, None);
        assert!(authority.session().focus_search_on_next_frame);
    }

    #[test]
    fn command_authority_clear_toggle_request_clears_one_shot_flag() {
        // The toggle-requested flag is a one-shot signal: set by the
        // keyboard handler, observed by the palette widget, then
        // cleared. Pin that clear is idempotent.
        let mut toggle = true;
        let mut session = CommandPaletteSession::default();

        let mut authority = CommandAuthorityMut {
            toggle_requested: &mut toggle,
            session: &mut session,
        };

        assert!(authority.toggle_requested());
        authority.clear_toggle_request();
        assert!(!authority.toggle_requested());
        // Idempotent — clearing an already-clear flag is a no-op.
        authority.clear_toggle_request();
        assert!(!authority.toggle_requested());
    }

    #[test]
    fn command_authority_reborrow_yields_distinct_handle_on_same_backing() {
        let mut toggle = false;
        let mut session = CommandPaletteSession::default();

        let mut outer = CommandAuthorityMut {
            toggle_requested: &mut toggle,
            session: &mut session,
        };

        {
            let mut inner = outer.reborrow();
            *inner.toggle_requested = true;
        }

        // Outer still observes the mutation because the reborrow
        // pointed at the same backing storage, not a separate copy.
        assert!(outer.toggle_requested());
    }
}
