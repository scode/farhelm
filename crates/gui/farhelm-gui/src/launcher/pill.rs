//! The two display-only reports a hidden launcher leaves at the bottom of the window: the progress pill while a slow
//! launch waits for its reply, and the "Started" report for a launch that succeeded without opening its session.
//!
//! SPEC.md, "Creation": "after about two seconds, a small pending pill appears at the bottom of the window with a
//! spinner, Launching (Restarting or Replacing for those modes), the captured host's name, the session name when
//! supplied, and elapsed seconds", announced once rather than every second, and a launch that does not open its session
//! shows "a display-only pill that says Started “name” for about four seconds". Ported from the old app's
//! `LaunchProgress` (`launch_progress.rs`) and its `started_report` (`list/view.rs`) at reference commit `24bc0580`,
//! with their `.launch-progress-pill` styles (`app.css` around line 6548). Here's the TLDR:
//!
//! - **One slot.** Both go through the layer manager's launch pill (`layers::show_launch_pill`), which places it centered
//!   20 pixels above the window's bottom edge, keeps it from taking focus or the pointer, and lets a later report
//!   replace an earlier one. [`super::launch`] decides when each shows and takes it down.
//! - **Captured words.** The progress pill names what the request names: the host and the name as they were at the
//!   press, never the draft's later edits or a host renamed meanwhile (old `PendingLaunch::new`: "Later form edits or
//!   host refreshes must not relabel an in-flight request").
//! - **Counting without announcing.** The pill is a status named by its words alone ("Launching “name” on host…"), so a
//!   screen reader says it once when it appears; the seconds beside it change every second without changing that name,
//!   as the old pill hid its ticking line from assistive technology.
//! - **Time from the ticker.** The seconds are worked out from the ticker's wall clock while the pill renders, and the
//!   pill holds the ticker's once-a-second clock to redraw, so tests move it with GPUI's simulated clock.

use std::time::{Duration, SystemTime};

use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::components::{Spinner, dialog_shadow, spinner_icon};
use crate::host_text::{HostSentence, HostText};
use crate::theme::{self, metrics};
use crate::ticker;

/// How long a launch waits before its progress pill shows: the old `LaunchProgress`'s two whole seconds, so a quick
/// reply never flashes a spinner.
pub(super) const PROGRESS_SHOWS_AFTER: Duration = Duration::from_secs(2);

/// How long the "Started" report stays up: the old report's four seconds.
pub(super) const STARTED_SHOWS_FOR: Duration = Duration::from_secs(4);

/// `.launch-progress-pill .launch-progress`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (10., 14.);
/// `.launch-progress-visible`'s gap between the spinner, the words and the seconds.
const GAP: f32 = 6.;
/// The old `.delete-spinner`'s size inside the pill.
const SPINNER: f32 = 12.;

/// The progress pill of one pending launch: its captured words, when it was sent, and the spinner and clock that redraw
/// it.
pub(super) struct ProgressPill {
    /// "Launching" or "Replacing" (Restart with adds "Restarting"): the old `LaunchAction::present`.
    verb: &'static str,
    host: HostText,
    /// The session name the request carries; `None` for a request that leaves the name to the helm.
    title: Option<HostText>,
    sent: SystemTime,
    /// Held only while the window is active, as every spinner is (`components::Spinner`): a live spinner
    /// keeps the ticker's timer firing ten times a second, which a window behind others has no use for. The seconds
    /// clock below runs either way, since a wait is about the user's time.
    spinner: Option<Spinner>,
    _seconds: ticker::Clock,
}

impl ProgressPill {
    /// The pill for a launch doing `verb` on `host` named `title`, sent at `sent` (the ticker's wall clock when the
    /// request went out, so the seconds count from then rather than from when the pill first shows).
    pub(super) fn new(
        verb: &'static str,
        host: HostText,
        title: Option<String>,
        sent: SystemTime,
        cx: &mut Context<Self>,
    ) -> ProgressPill {
        ProgressPill {
            verb,
            host,
            title: title
                .filter(|title| !title.is_empty())
                .map(HostText::from_host),
            sent,
            spinner: None,
            _seconds: ticker::every_second(cx),
        }
    }

    /// The pill's words without the seconds: "Launching “name” on host…" (or "Replacing ..."), or without the name when
    /// the request names none. The name is the user's typed text and the host the helm's, so both are drawn escaped as
    /// host values.
    fn words(&self) -> HostSentence {
        let mut words = HostSentence::new("launch-progress-label").text(self.verb);
        if let Some(title) = &self.title {
            words = words.text(" “").host(title).text("”");
        }
        words.text(" on ").host(&self.host).text("…")
    }
}

impl Render for ProgressPill {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::render_count::record("launch-progress", cx);
        let spins = ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        let palette = theme::palette(cx);
        let ink = theme::color(palette.fg_0);
        let seconds = ticker::wall_now(cx)
            .duration_since(self.sent)
            .map_or(0, |waited| waited.as_secs());
        let words = self.words();
        capsule("launch-progress", palette)
            .role(Role::Status)
            .aria_label(words.accessible_name())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .min_w_0()
            .child(spinner_icon(step, px(SPINNER), ink))
            .child(div().flex_1().min_w_0().overflow_hidden().child(words))
            .child(div().flex_none().child(format!("{seconds} s")))
    }
}

/// The "Started “name”" report for a launch that succeeded while the user was elsewhere, `title` being the new
/// session's title as the helm listed it.
pub(super) struct StartedReport {
    title: HostText,
}

impl StartedReport {
    pub(super) fn new(title: &str) -> StartedReport {
        StartedReport {
            title: HostText::from_host(title),
        }
    }
}

impl Render for StartedReport {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let words = HostSentence::new("launch-started-label")
            .text("Started “")
            .host(&self.title)
            .text("”");
        capsule("launch-started-report", palette)
            .role(Role::Status)
            .aria_label(words.accessible_name())
            .min_w_0()
            .overflow_hidden()
            .child(words)
    }
}

/// The capsule both reports are drawn in, under `id`: `.launch-progress-pill .launch-progress`'s border, fill, shadow
/// and padding, with the old 12-pixel text in `--fg-0`.
fn capsule(id: &'static str, palette: &theme::Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .debug_selector(move || id.to_owned())
        .py(px(PAD.0))
        .px(px(PAD.1))
        .border_1()
        .border_color(theme::color(palette.border_dialog))
        // `--radius-pill`: a capsule whatever the pill's height.
        .rounded_full()
        .bg(theme::color(palette.bg_2))
        .shadow(dialog_shadow(palette))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_0))
}
