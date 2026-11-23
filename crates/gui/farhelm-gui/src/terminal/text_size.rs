//! The terminal text size: one size for every open terminal, kept as the helm's shared `terminal_text_size`
//! preference, and the − / readout / + group that steps it.
//!
//! SPEC.md, "Terminal experience": the magnifier − / + buttons at the start of the session's top row's right-hand
//! group, or Cmd+Shift with + or − on macOS (Ctrl+Shift elsewhere, by the US-layout positions of = and −), "step every
//! open terminal's text size together, hidden tabs included, within a fixed range, and each terminal is resized so the
//! program inside sees its new rows and columns". The readout between the buttons is display-only and follows every
//! change. Ported from the old app's `TerminalTextSize` and `.terminal-text-size` (`session_view.rs` and `app.css` at
//! reference commit `24bc0580`), whose size the old app remembered per device: here it is a helm preference shared by
//! every client (the migration's decision), unset meaning the old default of 14 points. Here's the TLDR of the
//! mechanics:
//!
//! - **The size** is [`current`]: a step this client has sent and the helm has not answered yet ([`step`]), else the
//!   pushed preference, else 14. Every whole size from 9 to 28 is allowed ([`TERMINAL_TEXT_SIZE_RANGE`], the same
//!   range as [`TextSize`]); a step past either end does nothing.
//! - **Stepping** sends the new size as a keyless preference write ([`operations::send`]: nothing draws it busy) and
//!   holds it on screen until the helm answers the latest write, as the sidebar's width does, so held repeats step at
//!   once and the last write wins (the channel keeps commands in order). A refusal or a lost command ends the hold too,
//!   so the terminals then show what the helm stores; a failure is logged, never shown. The hold belongs to the UI
//!   session: a reset drops it. It is the second of the two values a view shows ahead of the store, which SPEC_impl.md
//!   ("The store, and how views redraw") allows, and it must stay as narrow as the sidebar width's: only the size this
//!   client stepped, only until the reply to its latest write.
//! - **The terminals** each follow [`current`] ([`crate::terminal::attached::AttachedTerminal`]'s store observer and
//!   this module's global), re-measure their cell when it changes, and refit, which resizes the program.
//! - **The shortcut** reaches [`step`] through the main view's window-wide handlers, whatever holds focus, and never
//!   reaches a program: the physical-key table binds it app-wide (`actions::PHYSICAL_KEYS`). It does nothing while no
//!   terminal is open.

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences, TERMINAL_TEXT_SIZE_RANGE};
use gpui::{
    AnyElement, App, BorrowAppContext as _, EntityId, Global, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div, px,
};

use super::TextSize;
use super::attached::AttachedTerminal;
use crate::assets::Icon;
use crate::components::{
    BUTTON_JOIN_GAP, Button, Condensed, HoverHelp, IconButton, JoinSide, TOOLBAR_TEXT, Tier,
};
use crate::desktop::title_band;
use crate::session;
use crate::{operations, theme};

/// The readout's width (`.terminal-text-size-readout`: `width: 22px`), fixed so the buttons stay still between one- and
/// two-digit sizes.
const READOUT_WIDTH: f32 = 22.;

/// The group's element id, and the readout's.
pub(crate) const GROUP_ID: &str = "terminal-text-size";
pub(crate) const READOUT_ID: &str = "terminal-text-size-readout";

/// The buttons' ids.
pub(crate) const SMALLER_ID: &str = "terminal-text-size-smaller";
pub(crate) const LARGER_ID: &str = "terminal-text-size-larger";

/// This client's step the helm has not answered yet. Terminals observe this global; it changes only through
/// [`App::update_global`], which tells them, and only when a step is held or released.
#[derive(Default)]
pub(crate) struct State {
    /// The latest step sent, by its number and size, until the helm answers it, and the UI session it was sent under:
    /// a hold from an earlier session counts for nothing, as a reset drops the reply that would end it.
    held: Option<(EntityId, u64, u8)>,
    /// How many steps were sent, which numbers the next.
    sent: u64,
}

impl Global for State {}

/// Every terminal made so far, which the shortcut needs at least one live of ([`any_terminal_open`]); dropped ones are
/// pruned as the next enrolls. A global of its own, apart from [`State`], so a terminal opening does not wake every
/// terminal's size observer: the terminals step by observing [`State`], never through this list.
#[derive(Default)]
struct Terminals(Vec<WeakEntity<AttachedTerminal>>);

impl Global for Terminals {}

/// The current UI session's entity id, which a hold is valid under.
fn session_id(cx: &App) -> Option<EntityId> {
    session::connection(cx).and_then(|connection| {
        connection
            .read(cx)
            .session()
            .map(|session| session.entity_id())
    })
}

/// The text size every open terminal is drawn at now: this client's step the helm has not answered yet, else the
/// pushed preference (when it is in range), else the old default of 14 points.
pub(crate) fn current(cx: &App) -> TextSize {
    let now = session_id(cx);
    let held = cx
        .try_global::<State>()
        .and_then(|state| state.held)
        .filter(|(session, _, _)| Some(*session) == now)
        .map(|(_, _, points)| points);
    let stored = session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.terminal_text_size)
            .filter(|points| TERMINAL_TEXT_SIZE_RANGE.contains(points))
            .and_then(|points| u8::try_from(points).ok())
    });
    held.or(stored).map_or(TextSize::DEFAULT, TextSize::new)
}

/// Count `terminal` among the open terminals the shortcut steps, as it is made. A terminal that is dropped falls out on
/// its own.
pub(crate) fn enroll(terminal: WeakEntity<AttachedTerminal>, cx: &mut App) {
    let terminals = &mut cx.default_global::<Terminals>().0;
    terminals.retain(|terminal| terminal.upgrade().is_some());
    terminals.push(terminal);
}

/// The open terminals, oldest first, for tests that read what each is drawn at.
#[cfg(test)]
pub(crate) fn open_terminals(cx: &App) -> Vec<gpui::Entity<AttachedTerminal>> {
    cx.try_global::<Terminals>()
        .map(|terminals| terminals.0.iter().filter_map(WeakEntity::upgrade).collect())
        .unwrap_or_default()
}

/// Whether any terminal is open, which the shortcut needs to do anything (SPEC.md: "While a terminal is open the
/// shortcut works wherever focus is").
pub(crate) fn any_terminal_open(cx: &App) -> bool {
    cx.try_global::<Terminals>().is_some_and(|terminals| {
        terminals
            .0
            .iter()
            .any(|terminal| terminal.upgrade().is_some())
    })
}

/// Step the text size by `delta` points, within the range: hold the new size on screen and send it as the helm's
/// `terminal_text_size`. A step past either end, or with no UI session, does nothing.
pub(crate) fn step(delta: i8, cx: &mut App) {
    let now = current(cx).points();
    let wanted = TextSize::new(now.saturating_add_signed(delta)).points();
    if wanted == now || session::current_store(cx).is_none() {
        return;
    }
    let Ok(path) = ApiPath::api(&["preferences"]) else {
        return;
    };
    let Some(under) = session_id(cx) else {
        return;
    };
    cx.default_global::<State>();
    let number = cx.update_global::<State, _>(|state, _| {
        state.sent += 1;
        state.held = Some((under, state.sent, wanted));
        state.sent
    });
    operations::send(
        Method::Put,
        path,
        Some(serde_json::json!({ "terminal_text_size": wanted })),
        "PUT /api/preferences",
        cx,
        move |answer, cx| {
            if let Err(reason) = answer {
                tracing::warn!(?reason, "could not save the terminal text size");
            }
            // A reply to anything but the latest step leaves the latest one held.
            let latest = cx
                .try_global::<State>()
                .and_then(|state| state.held)
                .is_some_and(|(_, held, _)| held == number);
            if latest {
                cx.update_global::<State, _>(|state, _| state.held = None);
            }
        },
    );
}

/// The − / readout / + group at the start of the toolbar. One-pixel gaps separate each tinted magnifier button from
/// the fixed-width readout of the current size in points. The readout is display-only, and the buttons step [`current`]
/// by one point. In the macOS window the group is
/// part of the title band, where the readout counts as text: a press on it neither drags nor zooms the window
/// ([`title_band::claims_presses`]; SPEC.md, "Desktop window chrome").
///
/// The three cells stretch to [`Button::toolbar_height`] so the group lines up with the lifecycle buttons in both
/// layouts. The ordinary magnifiers are square and the condensed ones keep their narrow width.
///
/// It keeps no state of its own, so the header can build it in both looks every frame and draw the one that fits.
pub(crate) fn controls(compact: bool, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    let points = current(cx).points();
    let height = Button::toolbar_height(compact.then_some(Condensed::Words));
    let magnifier = |button: IconButton, side| {
        let button = button.tier(Tier::Neutral).joined(side);
        if compact {
            button.compact()
        } else {
            button.toolbar().square(height)
        }
    };
    // Text in the macOS title band keeps its presses; the buttons claim theirs as controls already.
    let readout = title_band::claims_presses(div())
        .id(READOUT_ID)
        .debug_selector(|| READOUT_ID.into())
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w(px(READOUT_WIDTH))
        .h_full()
        .bg(theme::color(palette.button_rest))
        .text_color(theme::color(palette.fg_1))
        .text_size(if compact {
            theme::metrics::TEXT_SM
        } else {
            TOOLBAR_TEXT
        })
        .child(SharedString::from(points.to_string()));
    div()
        .id(GROUP_ID)
        .debug_selector(|| GROUP_ID.into())
        .role(Role::Group)
        .aria_label(format!("Terminal text size, {points} points"))
        .flex()
        .flex_row()
        .flex_none()
        .gap(BUTTON_JOIN_GAP)
        .h(height)
        .items_stretch()
        .child(
            magnifier(
                IconButton::new(
                    SMALLER_ID,
                    Icon::ZoomOut,
                    "Smaller terminal text",
                    HoverHelp::tooltip(format!("Smaller terminal text ({})", chord("−"))).below(),
                    Role::Button,
                ),
                JoinSide::Right,
            )
            .on_click(|_, _, cx| step(-1, cx)),
        )
        .child(readout)
        .child(
            magnifier(
                IconButton::new(
                    LARGER_ID,
                    Icon::ZoomIn,
                    "Larger terminal text",
                    HoverHelp::tooltip(format!("Larger terminal text ({})", chord("+"))).below(),
                    Role::Button,
                ),
                JoinSide::Left,
            )
            .on_click(|_, _, cx| step(1, cx)),
        )
        .into_any_element()
}

/// The shortcut's chord as the hover help names it on this platform (the old help's `Cmd/…` form, per platform).
fn chord(key: &str) -> String {
    if crate::actions::THIS_PLATFORM == farhelm_input_mapping::Platform::Mac {
        format!("⌘⇧{key}")
    } else {
        format!("Ctrl+Shift+{key}")
    }
}
