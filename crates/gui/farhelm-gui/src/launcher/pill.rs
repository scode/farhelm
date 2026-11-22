//! The display-only reports a hidden launcher leaves at the bottom of the window. Preparation starts as the user
//! presses Launch on a resumable host. The ordinary progress pill takes over once the create has waited two seconds.
//! A launch that succeeded without opening its session leaves a brief "Started" report.
//!
//! SPEC.md, "Creation": "after about two seconds, a small pending pill appears at the bottom of the window with a
//! spinner, Launching (Restarting or Replacing for those modes), the captured host's name, the session name when
//! supplied, and elapsed seconds", announced once rather than every second, and a launch that does not open its session
//! shows "a display-only pill that says Started “name” for about four seconds". Ported from the old app's
//! `LaunchProgress` (`launch_progress.rs`) and its `started_report` (`list/view.rs`) at reference commit `24bc0580`,
//! with their `.launch-progress-pill` styles (`app.css` around line 6548). Here's the TLDR:
//!
//! - **One slot.** Each goes through the layer manager's launch pill (`layers::show_launch_pill`), which places it centered
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
//! - **An environment create's steps.** Once the helm has accepted a create in a new environment and named the run it
//!   lands on (`launch::followed_run`), the pill names that run's running step instead ("Preparing <recipe> on
//!   <host>…", "Creating a <recipe> container on <host>…"), with the step's explanation beside it and in its accessible
//!   name, and the seconds counted from when the pill first saw the step (the first one it sees, from the press), read
//!   from the pushed host progress as it draws (SPEC.md: "Existing launch progress names … each with elapsed time"; the
//!   old `PendingLaunch::observe_stage`). The steps' words are the helm's and name a recipe and a host, so they are
//!   drawn as host text.

use std::time::{Duration, SystemTime};

use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use farhelm_proto::provisioning::{ProvisioningView, StepStatus};
use farhelm_proto::ui_channel::HostView;

use crate::components::{Spinner, dialog_shadow, spinner_icon};
use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};
use crate::{session, ticker};

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
    /// The run's step the pill names, and when it first saw that step running, which its seconds count from. Kept
    /// between steps, so the pill goes on naming the last one until the next starts, as the old pill did.
    stage: Option<(String, SystemTime)>,
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
            stage: None,
            spinner: None,
            _seconds: ticker::every_second(cx),
        }
    }

    /// The running step of the run the launch waits on, if it is an accepted environment create
    /// (`launch::followed_run`), as the store shows it now: its name and its explanation, if it has one.
    fn running_step(&self, cx: &gpui::App) -> Option<(String, Option<String>)> {
        let (host, run) = super::launch::followed_run(cx)?;
        let store = session::current_store(cx)?;
        let collections = store.read(cx).collections();
        let view = collections
            .get::<ProvisioningView>(&HostView::key(host))
            .filter(|view| view.run_id.as_deref() == Some(run.as_str()))?;
        let step = view
            .steps
            .iter()
            .find(|step| step.status == StepStatus::Running)?;
        Some((step.name.clone(), step.explanation.clone()))
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
        let now = ticker::wall_now(cx);
        // A step seen running for the first time starts the seconds over (the old `observe_stage`). Kept here, in the
        // pill's own state, because the helm publishes no step timestamps. The first step the pill sees counts from
        // the press instead: the run starts as the helm admits the launch, just after it, and the pill first draws two
        // seconds later. Strictly, the first step it sees may be a later one, if an earlier step ended before the pill
        // saw it running; that step is then counted from the press too, the earlier steps' time included.
        let running = self.running_step(cx);
        let detail = running
            .as_ref()
            .and_then(|(_, detail)| detail.clone())
            .map(HostText::from_host);
        if let Some((name, _)) = running
            && self.stage.as_ref().is_none_or(|(shown, _)| *shown != name)
        {
            let seen = if self.stage.is_none() { self.sent } else { now };
            self.stage = Some((name, seen));
        }
        let since = self.stage.as_ref().map_or(self.sent, |(_, seen)| *seen);
        let seconds = now
            .duration_since(since)
            .map_or(0, |waited| waited.as_secs());
        let words = match &self.stage {
            Some((name, _)) => {
                HostSentence::new("launch-progress-label").host(&HostText::from_host(name.as_str()))
            }
            None => self.words(),
        };
        // The step's explanation is part of what the status says, as it is part of what it shows.
        let name = match &detail {
            Some(detail) => {
                let detail = HostSentence::new("launch-progress-detail").host(detail);
                format!("{} {}", words.accessible_name(), detail.accessible_name())
            }
            None => words.accessible_name().to_string(),
        };
        capsule("launch-progress", palette)
            .role(Role::Status)
            .aria_label(SharedString::from(name))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .min_w_0()
            .child(spinner_icon(step, px(SPINNER), ink))
            .child(div().flex_1().min_w_0().overflow_hidden().child(words))
            .children(
                detail
                    .as_ref()
                    .map(|detail| host_text::element(detail).min_w_0().overflow_hidden()),
            )
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

/// The capsule each report is drawn in, under `id`: `.launch-progress-pill .launch-progress`'s border, fill, shadow
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
/// Preparation before the create travels to its host. This capsule never takes input or focus, so a switcher or a
/// different session can stay open while the destination wakes.
pub(super) struct WakePill {
    host: HostText,
    starting: bool,
    spinner: Option<Spinner>,
}

impl WakePill {
    pub(super) fn new(host: HostText, starting: bool) -> Self {
        Self {
            host,
            starting,
            spinner: None,
        }
    }
}

impl Render for WakePill {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spins = ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        let palette = theme::palette(cx);
        let words = HostSentence::new("launch-wake-label")
            .text(if self.starting {
                "Starting "
            } else {
                "Resuming "
            })
            .host(&self.host)
            .text(", then connecting… the session starts when it's ready");
        capsule("launch-wake", palette)
            .role(Role::Status)
            .aria_label(words.accessible_name())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .min_w_0()
            .child(spinner_icon(step, px(SPINNER), theme::color(palette.fg_0)))
            .child(div().min_w_0().overflow_hidden().child(words))
    }
}
