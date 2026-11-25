//! The notice center: the one place that decides when a notice is posted to, and withdrawn from, the surfaces that
//! show it.
//!
//! SPEC_impl.md, "Notices: the notice center": it is a code owner, not a new surface. The old app decided "should the
//! user hear about this now" in each feature. Here a feature reports what happened ([`post`], [`withdraw`]) and the
//! notice center alone keeps what is showing; the surfaces read [`posted`] when they draw. A notice is never shown by
//! its feature directly.
//!
//! The center holds notices for the window's approval region and banners. Session row bells and the rail's
//! Notifications panel draw the records that the helm pushes to the store. The sound owner decides when to play
//! status chimes.
//!
//! A notice can carry the host values it names. The upload banner keeps the original file names and session title
//! as host text so they remain safe to draw after the session row and terminal are gone.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{App, AppContext as _, Entity, Global, SharedString};

use crate::host_text::{HostSentence, HostText};

/// Everything the notice center can show. Exhaustive, so adding a notice makes every surface decide about it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    /// Files whose upload was cut off and whose session is no longer open. The banner outlives the terminal and
    /// connection that held the upload, so the user sees the result without returning to that session.
    InterruptedUpload(Rc<InterruptedUploads>),
    /// The helm refused an explicit Don't show again after the highlights dialog closed.
    UpgradeHighlightsOptOutFailed,
}

impl Notice {
    /// The notice's accessible text. An interrupted-upload banner draws the host parts from its sentence so each
    /// file name and session title keeps its direction isolation.
    pub fn text(&self) -> SharedString {
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
            Notice::InterruptedUpload(upload) => upload.sentence().accessible_name(),
            Notice::UpgradeHighlightsOptOutFailed => {
                "Could not turn off upgrade highlights. Try again under App in Settings.".into()
            }
        }
    }
}

/// One interruption of files with the same outcome from one session. Each occurrence has its own identity so a
/// second interruption with the same title and file names can be revealed and dismissed independently.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct InterruptedUploads {
    occurrence: u64,
    title: Option<HostText>,
    files: Vec<HostText>,
    sent: bool,
}

impl InterruptedUploads {
    /// Build a notice for a nonempty group. An empty title leaves out the destination clause. A sent request's
    /// outcome is unknown if the connection never answers.
    pub(crate) fn new(title: Option<HostText>, files: Vec<HostText>, sent: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self {
            occurrence: NEXT.fetch_add(1, Ordering::Relaxed),
            title: title.filter(|title| !title.raw().is_empty()),
            files,
            sent,
        }
    }

    /// The banner and its Dismiss button keep this identity across redraws.
    pub(crate) fn key(&self) -> u64 {
        self.occurrence
    }

    /// Name a few files, the original session title, and what is known about their publication.
    pub(crate) fn sentence(&self) -> HostSentence {
        const NAMED_FILES: usize = 3;
        let count = self.files.len();
        let mut sentence =
            HostSentence::new(("notice-interrupted-upload", self.key())).text("Attaching ");
        for (index, file) in self.files.iter().take(NAMED_FILES).enumerate() {
            if index != 0 {
                sentence = sentence.text(if index + 1 == count { " and " } else { ", " });
            }
            sentence = sentence.host(file);
        }
        match count.saturating_sub(NAMED_FILES) {
            0 => {}
            1 => sentence = sentence.text(" and 1 more file"),
            more => sentence = sentence.text(format!(" and {more} more files")),
        }
        if let Some(title) = &self.title {
            sentence = sentence.text(" to ").host(title);
        }
        sentence.text(match (self.sent, count == 1) {
            (true, true) => " was interrupted — whether it was attached cannot be told.",
            (true, false) => " was interrupted — whether they were attached cannot be told.",
            (false, true) => " was interrupted before it was sent, so nothing was uploaded.",
            (false, false) => " was interrupted before they were sent, so nothing was uploaded.",
        })
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
            notices.posted.retain(|posted| posted != &notice);
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
