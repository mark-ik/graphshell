/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Context menu — flat list of available actions, anchored to a
//! pointer position, opened on right-click.
//!
//! Per [`iced_command_palette_spec.md` §7.3](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)
//! and [`iced_composition_skeleton_spec.md` §7.3](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
//! triggered on mouse-right anywhere a target is identifiable;
//! actions sourced from `ActionRegistry::available_for(target, ...)`;
//! flat list, no submenus or nested categories (the egui-era two-tier
//! model is retired per the 2026-04-29 simplification).
//!
//! Slice 1 scope: signature-only scaffolding. The `Widget` impl is
//! deferred to the S4 sub-slice that wires the first context-menu
//! target (likely the canvas / tile-pane right-click).

use iced::Point;

/// One entry in a context menu. Disabled entries render with a
/// dimmed style and surface the `disabled_reason` on hover (per
/// [`iced_command_palette_spec.md` §3.4](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)).
#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    pub shortcut_hint: Option<String>,
    pub destructive: bool,
    pub disabled_reason: Option<String>,
}

impl ContextMenuEntry {
    /// Construct an enabled, non-destructive entry.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut_hint: None,
            destructive: false,
            disabled_reason: None,
        }
    }

    /// Mark this entry destructive — render with a warning style and
    /// route through the host's `ConfirmDialog` gate before dispatch.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Attach a keyboard-shortcut hint (rendered right-aligned).
    pub fn shortcut_hint(mut self, hint: impl Into<String>) -> Self {
        self.shortcut_hint = Some(hint.into());
        self
    }

    /// Mark this entry disabled. The provided reason is surfaced as a
    /// tooltip / hover description so the user understands why the
    /// action cannot fire (selection-set mismatch, missing capability,
    /// etc.).
    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }
}

/// A flat-list context menu anchored at a pointer position.
///
/// Slice 1: builder shape only. The `Widget` impl in S4 will read
/// every field below, so the lints are pre-silenced rather than
/// flagged as dead code today.
#[allow(dead_code)]
pub struct ContextMenu<Message> {
    pub(crate) entries: Vec<ContextMenuEntry>,
    pub(crate) anchor: Point,
    pub(crate) on_select: Option<Box<dyn Fn(usize) -> Message>>,
    pub(crate) on_dismiss: Option<Message>,
}

impl<Message> ContextMenu<Message> {
    /// Construct an empty menu anchored at `anchor` (typically the
    /// pointer position when the right-click fired).
    pub fn new(anchor: Point) -> Self {
        Self {
            entries: Vec::new(),
            anchor,
            on_select: None,
            on_dismiss: None,
        }
    }

    /// Append one entry.
    pub fn push(mut self, entry: ContextMenuEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Append every entry from an iterator.
    pub fn push_iter(mut self, iter: impl IntoIterator<Item = ContextMenuEntry>) -> Self {
        self.entries.extend(iter);
        self
    }

    /// Register the click handler. Receives the entry index; the host
    /// translates into the appropriate `HostIntent`.
    pub fn on_select(mut self, f: impl Fn(usize) -> Message + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Register the dismiss handler (Escape or click outside).
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }
}

// TODO(S4): impl iced::widget::Widget<Message, Theme, Renderer> for
// ContextMenu, plus From<ContextMenu<...>> for Element<...>. The
// Widget impl owns:
//   - layout: anchored card with screen-edge clamping
//   - draw: per-entry row, separator, shortcut hint, destructive style
//   - on_event: pointer hit-test → on_select; click outside / Escape →
//     on_dismiss; disabled entries swallow click without dispatch
//   - operate: arrow-key navigation, Enter dispatch, focus trap
//   - overlay: render above sibling content via iced::overlay so the
//     menu escapes parent clipping
