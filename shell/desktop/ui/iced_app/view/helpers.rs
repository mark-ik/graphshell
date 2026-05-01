//! Host helpers — hotkey detection, host-routed actions, URL shape
//! detection, UX-event emission. Phase D extraction from view/mod.rs.

use super::*;

/// Is this iced event the "focus the omnibar" hotkey?
/// Ctrl+L (Cmd+L on macOS via `Modifiers::command()`). Consumed at
/// the app level — never reaches the runtime's `HostEvent` translation.
pub(crate) fn is_omnibar_focus_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => c.as_ref().eq_ignore_ascii_case("l") && modifiers.command(),
        _ => false,
    }
}

/// Is this iced event the "open Command Palette" hotkey?
/// Ctrl+Shift+P (Zed/VSCode-shaped). Per
/// [`iced_command_palette_spec.md` §2.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
pub(crate) fn is_command_palette_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event the "open Node Finder" hotkey?
/// Ctrl+P **without** Shift (Zed/VSCode-shaped). Per
/// [`iced_node_finder_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
pub(crate) fn is_node_finder_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && !modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event an Escape keypress?
pub(crate) fn is_escape_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        })
    )
}

/// Is this iced event an ArrowDown keypress?
pub(crate) fn is_arrow_down_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
            ..
        })
    )
}

/// Is this iced event an ArrowUp keypress?
pub(crate) fn is_arrow_up_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
            ..
        })
    )
}

/// Does `s` look like a URL or bare hostname?
///
/// Per [`iced_omnibar_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md):
/// explicit scheme (`://`) → URL; no spaces + contains `.` → bare
/// host. Everything else → non-URL-shaped → route to Node Finder.
pub(crate) fn is_url_shaped(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if s.contains("://") {
        return true;
    }
    !s.contains(' ') && s.contains('.')
}

/// Slice 28 host-side intercept for `ActionId`s whose effect is
/// opening or toggling an iced-owned overlay or rearranging
/// host-side composition state. Returns `Some(Message)` when the
/// host should handle the action directly; `None` lets the caller
/// fall through to `HostIntent::Action` runtime dispatch.
///
/// Slice 31 extends this with Frame switcher routing — Frame
/// composition lives in `IcedApp` (`frame`, `inactive_frames`),
/// not the runtime, so Frame* actions intercept here.
pub(crate) fn host_routed_action(
    action_id: graphshell_core::actions::ActionId,
) -> Option<Message> {
    use graphshell_core::actions::ActionId;
    match action_id {
        ActionId::GraphCommandPalette => Some(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        }),
        // GraphRadialMenu was retired (see iced_command_palette_spec.md
        // §7.4). Re-introducing it is part of the input-subsystem
        // rework, not a host-route today.

        // Slice 31: Frame composition lives host-side.
        ActionId::FrameOpen => Some(Message::NewFrame),
        ActionId::FrameDelete => Some(Message::CloseCurrentFrame),
        // FrameSelect cycles to the next frame. The caller can pre-
        // compute the target index, but the simplest dispatch is a
        // sentinel: SwitchFrame(0) (the most-recently-backgrounded
        // frame). A future picker modal can route via SwitchFrame(idx)
        // for explicit selection.
        ActionId::FrameSelect => Some(Message::SwitchFrame(0)),
        // Slice 34: rename modal owns the active Frame's label.
        ActionId::FrameRename => Some(Message::FrameRenameOpen),

        // Slice 32: NodeCreate modal lives host-side; both NodeNew
        // and NodeNewAsTab open the same URL-input modal. The
        // pane-vs-tab distinction is downstream (the tab semantics
        // would route through workbench-routing once the pane
        // policy lands).
        ActionId::NodeNew | ActionId::NodeNewAsTab => Some(Message::NodeCreateOpen),
        _ => None,
    }
}

/// Emit a UX event onto the runtime's observer registry. Centralized
/// so every emission site has identical borrow shape — `&self.host.runtime`
/// is enough; emit() takes `&self`. Per
/// [`ux_observability`](
/// ../../../crates/graphshell-core/src/ux_observability.rs).
pub(crate) fn emit_ux_event(app: &IcedApp, event: graphshell_core::ux_observability::UxEvent) {
    app.host.runtime.ux_observers.emit(event);
}
