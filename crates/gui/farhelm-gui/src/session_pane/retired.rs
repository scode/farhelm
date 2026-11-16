//! The retired card: what the session pane shows in place of a retired containerized session's terminals, which it
//! never mounts.
//!
//! SPEC.md, "Retired containerized sessions": such a session supports Delete only, and opening terminals or tabs is
//! refused; "Session view": "Retired containerized sessions offer no terminal or tab access". Ported from the old app's
//! `.retired-container-card` (`session_view.rs` and `app.css` at the reference commit), which carried the classes of
//! the interrupted card and so its look: a centered neutral box on `--bg-1` with the ordinary border, as tall as the
//! area between the top row and the details bar, holding the reason ([`farhelm_proto::RETIRED_CONTAINER_REASON`]).
//! Which sessions are retired, and what the list withdraws from them, is `session_list::retired`'s. Here's the TLDR of
//! the rules:
//!
//! - **No terminal, ever.** The pane mounts no terminal set for a retired session, so no terminal stream opens and the
//!   top row draws no tab strip, and it drops a set it made before the session's row said so (`session_pane.rs`).
//! - **When the card shows.** While the store holds the open session's row as retired on a host the helm reaches
//!   ([`shows_row`]). On a host it cannot reach the unavailable card stands in the terminal area alone, as it does for
//!   an interrupted session: stale beats retired, so the area never holds two cards at once.
//! - **The header** draws Delete alone, with its ordinary question; Stop, Restart, Replace, Clone and the terminal text
//!   size controls are not drawn (`header.rs`). A refused Delete shows below the top row as any refusal does.
//! - **Focus.** The card is home while it shows, as the other cards that stand where a terminal would be (SPEC.md,
//!   "Where focus rests"): a keyboard stop named by the reason, with nothing bound to Enter, so typing already under
//!   way does nothing. The old card was no stop; SPEC.md's rule was written after the old app. It shares the pane's one
//!   card focus handle with the interrupted and unavailable cards, since a session shows one of them at most.

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement as _, IntoElement, ParentElement, Role,
    Styled, Window, div, px, relative,
};

use super::card::{GAP, MAX_WIDTH, PAD};
use crate::components::{self, Ring};
use crate::session;
use crate::session_list::retired;
use crate::theme::{self, metrics};

/// The card's element id, by which tests and the bridge find it.
pub(super) const CARD_ID: &str = "session-retired-card";

/// Whether `key`'s row, as the current UI session's store holds it, is a retired containerized session's, whose
/// terminals the pane never mounts, stale or not. `false` while the store holds no row.
pub(super) fn is(key: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .is_some_and(retired::is)
    })
}

/// Whether `row` shows the retired card: it is retired and the helm reaches its host (see the module docs, "When the
/// card shows").
pub(super) fn shows_row(row: &SessionRow) -> bool {
    !row.stale && retired::is(row)
}

/// [`shows_row`] for the row the current UI session's store holds under `key`; `false` while it holds none.
pub(super) fn shows(key: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .is_some_and(shows_row)
    })
}

/// The card, centered in the area the terminals would take, tracking `focus` (home while it shows).
pub(super) fn draw(focus: &FocusHandle, window: &Window, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    let reason = farhelm_proto::RETIRED_CONTAINER_REASON;
    let card = div()
        .id(CARD_ID)
        .debug_selector(|| CARD_ID.into())
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .max_w(px(MAX_WIDTH))
        .h_full()
        .gap(px(GAP))
        .p(px(PAD))
        .bg(theme::color(palette.bg_1))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_SM)
        .text_center()
        .text_color(theme::color(palette.fg_0))
        .child(
            div()
                .w_full()
                .text_size(metrics::TEXT_MD)
                .line_height(relative(1.5))
                .child(reason),
        );
    // Named by the reason, so assistive technology reads it where focus rests, with the keyboard's ring over the card's
    // own edge, as the interrupted card's.
    let ring = Ring {
        inset_x: px(0.),
        inset_y: px(0.),
        radius: metrics::RADIUS_SM,
    };
    let card = components::focus_stop(
        card,
        CARD_ID,
        focus,
        Role::Group,
        reason.into(),
        ring,
        window,
        cx,
    );
    div()
        .size_full()
        .flex()
        .justify_center()
        .child(card)
        .into_any_element()
}
