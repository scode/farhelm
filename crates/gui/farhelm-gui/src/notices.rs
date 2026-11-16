//! The notice center: the one place that decides when a notice is posted to, and withdrawn from, the surfaces that
//! show it.
//!
//! SPEC_impl.md, "Notices: the notice center": it is a code owner, not a new surface. The old app decided "should the
//! user hear about this now" in each feature. Here a feature reports what happened ([`post`], [`withdraw`]) and the
//! notice center alone keeps what is showing; the surfaces read [`posted`] when they draw. A notice is never shown by
//! its feature directly.
//!
//! Its surfaces are approval cards and banners, its own generic notices among them. Banners still to come, such as the
//! version-mismatch and outage cards, arrive with the areas that port them: each adds a [`Notice`] variant and the
//! surface that draws it. The session rows' bells with their lists, and the rail's notices panel when it comes, are
//! not its surfaces: they draw the notification records the helm keeps and pushes, straight from the store. Nor are the
//! status chimes, which the one sound owner decides. The migration's orchestrator drew that line on 2026-11-14.

use gpui::{App, AppContext as _, Entity, Global, SharedString};

/// Everything the notice center can show. Exhaustive, so adding a notice makes every surface decide about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Notice {
    /// The connection to the helm was lost and the window was rebuilt (SPEC.md, "Losing the connection to the helm").
    /// Stays until the user dismisses it.
    ConnectionLost,
    /// A launch the user had moved on from was refused, and its launcher waits with the draft and the refusal
    /// (SPEC.md, "Creation": "Otherwise a notice offers it, and the launcher returns when the user opens that notice").
    /// The banner's Open launcher brings it back (`launcher::launch`). The launcher withdraws it when it returns or a
    /// reset drops it.
    LaunchNotStarted,
    /// An answer to an agent's approval card arrived after its request had stopped waiting (answered elsewhere,
    /// expired, or its session gone), so it did not take effect. `always` is set after an Always allow, whose host
    /// setting the helm stores before it looks for the request. Drawn by the approval region above its waiting
    /// list, as the old app drew it, never as a banner; it stays until the user dismisses it, and a later late answer
    /// replaces it.
    LateAnswer { always: bool },
    /// A launch the user had moved on from got the helm's timeout reply: the host did not answer in time, so the create
    /// may still happen, and its launcher waits with the draft and the reply. Offered and withdrawn as
    /// [`Notice::LaunchNotStarted`] is, without saying the launch did not start.
    LaunchMayStillStart,
    /// A launch into a new environment (a container or a sandbox) that the user had moved on from ended in a way the
    /// launcher cannot learn: its run says it ended without a result, or went away, or the helm never named it. The
    /// environment may exist, so the notice says to check Hosts rather than that nothing launched; its launcher waits
    /// with the draft, offered and withdrawn as [`Notice::LaunchNotStarted`] is.
    EnvironmentLaunchUnknown,
}

impl Notice {
    /// The notice's text, as SPEC.md words it.
    /// The environment launch's notice is the app's own words: SPEC.md words none for it.
    pub fn text(self) -> SharedString {
        match self {
            Notice::ConnectionLost => {
                "The connection to the helm was lost; something you did just before may or may \
                                       not have happened."
                    .into()
            }
            Notice::LaunchNotStarted => {
                "Not launched. Open the launcher to see why; it still holds your settings.".into()
            }
            Notice::LateAnswer { always: false } => {
                "Your answer arrived after its request had stopped waiting, so it did not take effect.".into()
            }
            Notice::LateAnswer { always: true } => {
                "Your answer arrived after its request had stopped waiting, so it did not take effect. The \
                 host's permission for this kind of request may still have been turned on; its settings show \
                 whether it was."
                    .into()
            }
            Notice::LaunchMayStillStart => {
                "A launch timed out, so the session may still start. Open the launcher to see the reply; it still holds \
                 your settings."
                    .into()
            }
            Notice::EnvironmentLaunchUnknown => {
                "Farhelm could not learn how a launch into a new environment ended. Check Hosts before launching again; \
                 the launcher still holds your settings."
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

/// The notice center's entity, for a surface to observe. `None` only without [`install`], a bootstrap bug (see
/// [`crate::bootstrap::installed`]); posting and withdrawing then do nothing.
pub fn entity(cx: &App) -> Option<Entity<Notices>> {
    crate::bootstrap::installed::<NoticesGlobal>(cx).map(|global| global.0.clone())
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
