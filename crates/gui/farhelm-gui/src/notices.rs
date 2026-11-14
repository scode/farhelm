//! The notice center: the one place that decides when a notice is posted to, and withdrawn from, the surfaces that
//! show it.
//!
//! SPEC_impl.md, "Notices: the notice center": it is a code owner, not a new surface. The old app decided "should the
//! user hear about this now" in each feature, and the same event could reach the bell list, a banner and a chime by
//! three different rules. Here a feature reports what happened ([`post`], [`withdraw`]) and the notice center alone
//! keeps what is showing; the surfaces read [`posted`] when they draw.
//!
//! NOTE: Only the one notice the reset needs exists so far, the generic connection-lost notice, shown as a banner at
//! the top of the window. The bell list and notices panel, approval cards, the version-mismatch and outage cards, and
//! chimes arrive with the areas that port them: each adds a [`Notice`] variant and the surface that draws it. A notice
//! is never shown by its feature directly.

use gpui::{App, AppContext as _, Entity, Global, SharedString};

/// Everything the notice center can show. Exhaustive, so adding a notice makes every surface decide about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Notice {
    /// The connection to the helm was lost and the window was rebuilt (SPEC.md, "Losing the connection to the helm").
    /// Stays until the user dismisses it.
    ConnectionLost,
}

impl Notice {
    /// The notice's text, as SPEC.md words it.
    pub fn text(self) -> SharedString {
        match self {
            Notice::ConnectionLost => {
                "The connection to the helm was lost; something you did just before may or may \
                                       not have happened."
                    .into()
            }
        }
    }
}

/// What is showing, in the order it was posted. A notice shows at most once.
#[derive(Default)]
pub struct Notices {
    posted: Vec<Notice>,
}

/// The notice center's entity, which surfaces observe so a post or a withdrawal redraws them.
struct NoticesGlobal(Entity<Notices>);

impl Global for NoticesGlobal {}

/// Install the notice center. Part of the bootstrap.
pub fn install(cx: &mut App) {
    let notices = cx.new(|_| Notices::default());
    cx.set_global(NoticesGlobal(notices));
}

/// The notice center's entity, for a surface to observe.
pub fn entity(cx: &App) -> Option<Entity<Notices>> {
    cx.try_global::<NoticesGlobal>()
        .map(|global| global.0.clone())
}

/// Post `notice`. Posting one that is already showing changes nothing.
pub fn post(notice: Notice, cx: &mut App) {
    if let Some(notices) = entity(cx) {
        notices.update(cx, |notices, cx| {
            if !notices.posted.contains(&notice) {
                notices.posted.push(notice);
                cx.notify();
            }
        });
    }
}

/// Withdraw `notice`, because the user dismissed it or it no longer applies.
pub fn withdraw(notice: Notice, cx: &mut App) {
    if let Some(notices) = entity(cx) {
        notices.update(cx, |notices, cx| {
            let before = notices.posted.len();
            notices.posted.retain(|posted| *posted != notice);
            if notices.posted.len() != before {
                cx.notify();
            }
        });
    }
}

/// What is showing now, oldest first.
pub fn posted(cx: &App) -> Vec<Notice> {
    entity(cx).map_or_else(Vec::new, |notices| notices.read(cx).posted.clone())
}
