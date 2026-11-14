//! The main pane beside the session list, where the open session's terminal will be.
//!
//! For now it shows only what the old app showed with no session open (its `.main-empty`), and the old rule for it:
//! three honest states, not one claim. It says "No sessions — create one" only once the helm has shown that there are
//! none ([`crate::session_list::empty_fleet`]); before the session list has been read it says "Loading sessions…"; and
//! when there are sessions it is blank, because the session list is about to open one (or has, and its terminal comes
//! with a later piece of the migration).

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::Collection;
use gpui::{App, IntoElement, ParentElement, Styled, div};

use crate::theme;
use crate::{session, session_list};

/// The pane's content.
pub fn element(cx: &App) -> impl IntoElement {
    let store = session::current_store(cx);
    let text = placeholder(store.as_ref().map(|store| store.read(cx).collections()));
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::color(theme::palette(cx).fg_2))
        .child(text)
}

/// What the pane says over `collections` (`None` while the window is inert, when the window shows its connection state
/// instead and the pane is not drawn).
pub(crate) fn placeholder(collections: Option<&Collections>) -> &'static str {
    match collections {
        None => "",
        Some(collections) if collections.meta(Collection::Sessions).is_none() => {
            "Loading sessions…"
        }
        Some(collections) if session_list::empty_fleet(collections) => "No sessions — create one",
        Some(_) => "",
    }
}
