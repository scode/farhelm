//! The provisioning run a host row shows: the inline status in place of the phase words, the step list under the row,
//! and the two facts this UI keeps about each run (when it first saw the run, and whether the user folded it).
//!
//! SPEC.md, "Topology" (the inline status's current step, completed-step count and client-measured elapsed time; the
//! step colors; "Confirming closes the dialog, and the running row expands once ... The user may fold that run ...
//! Success folds the row, while failure opens it for the step and its reason"). Ported from the old app's `hosts.rs`
//! (`UpdateProgressLabel`, `UpdateProgressDot`, `format_elapsed`) and `provisioning.rs` (`summarize_running_view`,
//! `confirmed_run_disclosure`, `progress_step_status`, `run_display_label`, the panel's `.provisioning-run` block),
//! with the look of the `.host-update-*` and `.provisioning-*` rules in `app.css`, all at reference commit `24bc0580`.
//! Here's the TLDR of its rules:
//!
//! - **Setup, Update and uninstall runs are drawn** ([`shown`], [`Run`]).
//! - **The inline status** ([`status`]): "Setting up" (or "Uninstalling"), the completed-step count over the step
//!   count, the current step (the first running one, by the helm's own name for it) and the elapsed time, beside a
//!   pulsing amber dot. An Update's says the same without the visible verb ("Updating" is read out, not drawn), as the
//!   old label kept Update's narrower. While a rerun's probe or its confirmed setup is out and no run shows yet,
//!   "Setting up…"; while an Update's plan or its confirming request is out, "Updating…" (SPEC.md: "`Updating…` until
//!   a progress snapshot is available"); while an uninstall's plan or its confirming request is out, "Uninstalling…"
//!   (SPEC.md: "Setup and uninstall planning show their action in the folded row while the helm prepares a dialog").
//! - **The step list** ([`run_section`]) shows under an open row for any retained run, running, failed or completed, as
//!   the old panel did: each step's number, check or cross on a rail, its name and its result word, and the step's and
//!   the run's messages, which carry the helm's reason for a failure.
//! - **Disclosure** ([`opens_row`]): a running setup or uninstall opens its row by itself until the user folds that run
//!   with Collapse; success closes it again (a finished uninstall takes the row with it), and a failure keeps it open
//!   whatever the fold said (the Hosts panel's `disclosed` adds the failure rule for every kind of run). A running
//!   Update never opens its row (SPEC.md: "While an update is running, its host row stays folded"), so it offers no
//!   Collapse.
//! - **What the UI keeps** ([`Seen`]): per host, the run it last saw running, when it first saw it, and the fold. The
//!   helm's progress carries no timestamps, so the elapsed time counts from when this UI session first saw the run
//!   running, as the old app counted from its own first sight (a reload started it over; here a reset does). The
//!   settings-and-hosts goal (M2.9) caps what the UI holds about a run to the fold; the time it first saw a run is
//!   the one other fact, because SPEC.md asks for an elapsed time measured by the client and nothing else can give one.
//! - **The progress card** ([`card`]): the hover card of a running run, which the Hosts row's inline status and the
//!   attention list's working rows share. It draws its steps with the step list's own rows.
//!
//! ## Where what the UI keeps lives
//!
//! The [`Seen`] records live in one app global for the current UI session ([`follow_current`], [`fold_current`],
//! [`seen_now`]), tagged with the entity id of the operation store they were made under, as [`super::status`] keeps
//! its warnings, so a reset forgets them. A global rather than a field of the Hosts panel, because the attention list's
//! card counts the same run's elapsed time as the Hosts row does (SPEC.md: hovering the status "opens the same card as
//! a working host in the attention list"), and the attention list has no handle on the panel. The panel's store
//! subscription keeps it up to date on every change of the store, whether or not Hosts shows.
//!
//! Time comes from the ticker, shared by every row whatever its run. The running marker and the inline dot pulse on the
//! shared pulse ([`crate::ticker::pulse`]), which the panel holds only while its window is active and which stands
//! still under reduced motion. The elapsed time is read from [`crate::ticker::wall_now`] whenever the panel draws, and
//! the panel holds the ticker's one-second clock ([`crate::ticker::every_second`]) while a setup's or an Update's run
//! shows, because the clock must still count in an inactive window and under reduced motion, where the pulse redraws
//! nothing. That clock's beat is shared with every other count of seconds in the app rather than set by this run, so a
//! run first seen mid-second turns over up to a second after its own whole second, as the ticker allows for every
//! count.

pub(crate) mod card;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use farhelm_proto::provisioning::{
    ProvisioningOperation, ProvisioningView, RunStatus, StepStatus, StepView,
};
use gpui::{
    AnyElement, App, EntityId, FontWeight, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px, svg,
};

use crate::assets::{self, Icon};
use crate::components::{Button, HoverHelp, StatusMark, Tier};
use crate::host_text::{self, HostSentence, HostText};
use crate::operations::Work;
use crate::session;
use crate::theme::{self, Palette, Token, metrics};
use crate::ticker;

/// `.host-update-dot`: the inline status's dot.
const DOT: f32 = 6.;

/// `.host-update-running`'s gap between the dot, the verb, the count, the step and the clock.
const STATUS_GAP: f32 = 4.;

/// `.provisioning-step-marker`: the circle holding a step's number, check or cross, and its column
/// (`grid-template-columns: 18px ...`) and the gap after it.
const MARKER: f32 = 17.;
const MARKER_COLUMN: f32 = 18.;
const STEP_GAP: f32 = 7.;

/// The line height of the step list's 12-pixel text: the old `normal` line height, as the panel's detail lines use.
const LINE: f32 = 15.;

// ---- What the UI keeps ----------------------------------------------------------------------------------------------

/// What this UI session keeps about one host's run: which run, when it first saw it running, and whether the user
/// folded it. Kept by host id in the current UI session's app global ([`follow_current`], [`seen_now`]), which a reset
/// starts over.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Seen {
    run_id: String,
    since: SystemTime,
    folded: bool,
}

/// Bring `seen` up to date with the host progress the helm pushes now (`runs`), at `now`. A running run not seen before
/// is recorded as first seen now, unfolded; a host whose retained run went or is another run forgets the old one, so a
/// new run starts open and with its own clock (the old rule: "the run id is the boundary: folding one run cannot hide
/// the next one").
pub(crate) fn follow<'a>(
    seen: &mut HashMap<i64, Seen>,
    runs: impl IntoIterator<Item = &'a ProvisioningView>,
    now: SystemTime,
) {
    let runs: HashMap<i64, &ProvisioningView> =
        runs.into_iter().map(|run| (run.host_id, run)).collect();
    seen.retain(|host, kept| {
        runs.get(host).and_then(|run| run.run_id.as_deref()) == Some(kept.run_id.as_str())
    });
    for (host, run) in runs {
        if let (RunStatus::Running, Some(id)) = (&run.status, &run.run_id) {
            seen.entry(host).or_insert_with(|| Seen {
                run_id: id.clone(),
                since: now,
                folded: false,
            });
        }
    }
}

/// Fold host `host`'s run `run_id`, as its Collapse does: only that run, and only while it is the one `seen` holds.
pub(crate) fn fold(seen: &mut HashMap<i64, Seen>, host: i64, run_id: &str) {
    if let Some(kept) = seen.get_mut(&host)
        && kept.run_id == run_id
    {
        kept.folded = true;
    }
}

/// The [`Seen`] records of the UI session whose operation store is `store`, as an app global ([`follow_current`]).
#[derive(Default)]
struct Kept {
    store: Option<EntityId>,
    seen: HashMap<i64, Seen>,
}

impl Global for Kept {}

/// The current UI session's records, by host id; empty while the window is inert, before the first follow, and right
/// after a reset, until the panel follows the new session's store.
pub(crate) fn seen_now(cx: &App) -> HashMap<i64, Seen> {
    super::current_operations_id(cx)
        .and_then(|store| {
            cx.try_global::<Kept>()
                .filter(|kept| kept.store == Some(store))
        })
        .map(|kept| kept.seen.clone())
        .unwrap_or_default()
}

/// Change the current UI session's records with `change`, starting them over when they were made under another
/// session, and store them only when they changed, as [`super::status`] stores its state, so the global's observers
/// (none yet) would hear only of a change. Nothing while the window is inert.
fn change_current(cx: &mut App, change: impl FnOnce(&mut HashMap<i64, Seen>)) {
    let Some(store) = super::current_operations_id(cx) else {
        return;
    };
    let before = cx
        .try_global::<Kept>()
        .filter(|kept| kept.store == Some(store))
        .map(|kept| &kept.seen);
    let mut seen = before.cloned().unwrap_or_default();
    change(&mut seen);
    if before != Some(&seen) {
        cx.set_global(Kept {
            store: Some(store),
            seen,
        });
    }
}

/// [`follow`] the current store's host progress into the current UI session's records, at the ticker's time of day.
/// The Hosts panel calls it on every change of the store, whether or not Hosts shows, so a run's clock starts when it
/// is first pushed.
pub(crate) fn follow_current(cx: &mut App) {
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let now = ticker::wall_now(cx);
    let runs: Vec<ProvisioningView> = store
        .read(cx)
        .collections()
        .ordered::<ProvisioningView>()
        .into_iter()
        .cloned()
        .collect();
    change_current(cx, |seen| follow(seen, &runs, now));
}

/// [`fold`] host `host`'s run `run_id` in the current UI session's records, as its Collapse does.
pub(crate) fn fold_current(host: i64, run_id: &str, cx: &mut App) {
    change_current(cx, |seen| fold(seen, host, run_id));
}

// ---- Which runs show, and what they open ----------------------------------------------------------------------------

/// The kinds of run a row draws, which decide its words: the helm's setup (`add`), Update and uninstall operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Run {
    Setup,
    Update,
    Uninstall,
}

impl Run {
    /// The kind of a run of `operation`, or `None` for a run without one (the helm's idle view).
    fn of(operation: Option<ProvisioningOperation>) -> Option<Run> {
        match operation? {
            ProvisioningOperation::Add => Some(Run::Setup),
            ProvisioningOperation::Update => Some(Run::Update),
            ProvisioningOperation::Uninstall => Some(Run::Uninstall),
        }
    }

    /// The verb the inline status reads out ("Setting up: 1 of 3 steps done"), and the progress card's count starts
    /// with.
    fn verb(self) -> &'static str {
        match self {
            Run::Setup => "Setting up",
            Run::Update => "Updating",
            Run::Uninstall => "Uninstalling",
        }
    }

    /// Whether the inline status draws its verb before the count. The old label drew "Setting up" and "Uninstalling"
    /// but left Update's at the count (`.host-update-verb`), so an Update's status reads its verb out without drawing
    /// it.
    fn draws_verb(self) -> bool {
        match self {
            Run::Setup | Run::Uninstall => true,
            Run::Update => false,
        }
    }

    /// Whether a running run of this kind opens its row by itself until the user folds it: setup and uninstall, whose
    /// running row "expands once to show the helm's steps" (SPEC.md, "Topology"), never an Update, whose row "stays
    /// folded".
    fn opens_its_row(self) -> bool {
        match self {
            Run::Setup | Run::Uninstall => true,
            Run::Update => false,
        }
    }

    /// The step list's accessible name.
    fn steps(self) -> &'static str {
        match self {
            Run::Setup => "Setup steps",
            Run::Update => "Update steps",
            Run::Uninstall => "Uninstall steps",
        }
    }

    /// The step list's title (the old "<operation> provisioning").
    fn title(self) -> &'static str {
        match self {
            Run::Setup => "Setup provisioning",
            Run::Update => "Update provisioning",
            Run::Uninstall => "Uninstall provisioning",
        }
    }
}

/// The host's retained run, if it is one a row draws: a run with a run id and an operation (the helm's idle view has
/// neither).
pub(crate) fn shown(run: Option<&ProvisioningView>) -> Option<&ProvisioningView> {
    run.filter(|run| run.run_id.is_some() && Run::of(run.operation).is_some())
}

/// Whether `run` is a setup or an uninstall going on now that the user has not folded (by `seen`): such a run opens
/// its row by itself. An Update never does (the module docs).
pub(crate) fn opens_row(run: Option<&ProvisioningView>, seen: Option<&Seen>) -> bool {
    shown(run).is_some_and(|run| {
        Run::of(run.operation).is_some_and(Run::opens_its_row)
            && run.status == RunStatus::Running
            && !folded(run, seen)
    })
}

/// Whether the user folded this very run.
fn folded(run: &ProvisioningView, seen: Option<&Seen>) -> bool {
    seen.is_some_and(|kept| kept.folded && run.run_id.as_deref() == Some(kept.run_id.as_str()))
}

// ---- The inline status ----------------------------------------------------------------------------------------------

/// What a row's inline status says, in place of its phase words.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Status {
    /// Work on the host is out (a rerun's probe or its confirmed setup, an Update's or an uninstall's plan or its
    /// confirming request, before the helm accepts it) and no run shows yet: "Setting up…", "Updating…" or
    /// "Uninstalling…" alone.
    Pending(Run),
    /// A run shows: how many steps are done of how many, its current step, and how long since this UI first saw it
    /// running.
    Running {
        run: Run,
        done: usize,
        total: usize,
        current: Option<HostText>,
        elapsed: Duration,
    },
}

/// The inline status of a host whose retained run is `run`, with `seen` what the UI keeps about it and `in_flight` the
/// work out under the host's key, at `now`; `None` when the row shows its phase words. A step counts as done once it
/// completed, was unchanged, ended with a warning or failed; a status word this build does not know cannot reach here,
/// since the GUI decodes the helm's progress into a closed type.
pub(crate) fn status(
    run: Option<&ProvisioningView>,
    seen: Option<&Seen>,
    in_flight: Option<Work>,
    now: SystemTime,
) -> Option<Status> {
    if let Some((kind, run)) = shown(run)
        .filter(|run| run.status == RunStatus::Running)
        .and_then(|run| Some((Run::of(run.operation)?, run)))
    {
        let (done, current) = counts(run);
        let since = seen
            .filter(|kept| run.run_id.as_deref() == Some(kept.run_id.as_str()))
            .map_or(now, |kept| kept.since);
        return Some(Status::Running {
            run: kind,
            done,
            total: run.steps.len(),
            current,
            elapsed: now.duration_since(since).unwrap_or_default(),
        });
    }
    // Exhaustive, so a new kind of work decides here whether a row shows it as a run about to start. Only the requests
    // that lead to a setup, an Update or an uninstall run do; everything else (a host's Retry and Adopt, its removal, a
    // save, and the session, tab, tunnel, question, feedback and trash work) starts no run, so the row keeps its phase
    // words.
    match in_flight? {
        Work::Probe | Work::Setup => Some(Status::Pending(Run::Setup)),
        Work::UpdateHost => Some(Status::Pending(Run::Update)),
        Work::UninstallHost => Some(Status::Pending(Run::Uninstall)),
        Work::RetryHost
        | Work::AdoptHost
        | Work::RemoveHost
        | Work::Save
        | Work::Rename
        | Work::Delete(_)
        | Work::Stop
        | Work::Replace
        | Work::Seen
        | Work::Restart
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::Launch
        | Work::Answer
        | Work::SendFeedback
        | Work::TrashDelete
        | Work::TabOpen
        | Work::TabClose
        | Work::Pin
        | Work::MovePin
        | Work::Suspend
        | Work::Resume
        | Work::StopAndSuspend => None,
    }
}

/// How many of `run`'s steps are done, and its current step (the first running one, by the helm's own name for it). A
/// step counts as done once it completed, was unchanged, ended with a warning or failed. Shared by the row's inline
/// status and the rail's live work ([`super::status::live`]), so the two surfaces count a run alike.
pub(crate) fn counts(run: &ProvisioningView) -> (usize, Option<HostText>) {
    let done = run
        .steps
        .iter()
        .filter(|step| !matches!(step.status, StepStatus::Pending | StepStatus::Running))
        .count();
    let current = run
        .steps
        .iter()
        .find(|step| step.status == StepStatus::Running)
        .map(|step| HostText::from_host(step.name.as_str()));
    (done, current)
}

/// An elapsed time as the old status showed it: `m:ss`, or `h:mm:ss` from an hour on, never capped.
pub(crate) fn clock(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let (hours, minutes, seconds) = (seconds / 3_600, (seconds / 60) % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Host `id`'s inline status (`.host-update-running`): drawn under `host-status-<id>` like the phase words it replaces,
/// as a status named for its count and step (the step name is cut short on screen, as the old label's was). While a
/// run is going on, its hover is the progress card beside the sidebar ([`card_help`]), whose words, naming the host by
/// `name`, are also the status's accessible description; while only its requests are out ("Setting up…"), there are no
/// steps to show, and the hover is the words, as the old label had no card then. The dot is at `pulse`'s opacity. A
/// setup and an uninstall draw their verb before the count; an Update does not, though its name and hover still start
/// with it ([`Run::draws_verb`]).
pub(crate) fn status_mark(
    id: i64,
    name: &HostText,
    status: &Status,
    pulse: f32,
    palette: &Palette,
) -> AnyElement {
    let dot = div()
        .flex_none()
        .size(px(DOT))
        .rounded_full()
        .bg(theme::color(palette.warn))
        .when(pulse < 1., |dot| dot.opacity(pulse));
    let line = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(STATUS_GAP))
        .min_w_0()
        .whitespace_nowrap()
        .child(dot);
    let sentence = HostSentence::new(SharedString::from(format!("host-progress-{id}")));
    let (line, sentence, help) = match status {
        Status::Pending(run) => {
            let words = format!("{}…", run.verb());
            let sentence = sentence.text(words.clone());
            (
                line.child(words),
                sentence.clone(),
                HoverHelp::sentence(sentence),
            )
        }
        Status::Running {
            run,
            done,
            total,
            current,
            elapsed,
        } => {
            let count = format!("{done}/{total}");
            let sentence = sentence.text(format!("{}: {done} of {total} steps done", run.verb()));
            let sentence = match current {
                Some(step) => sentence.text(", now ").host(step),
                None => sentence,
            };
            let verb = run
                .draws_verb()
                .then(|| div().flex_none().child(run.verb()));
            let line = line
                .children(verb)
                .child(div().flex_none().child(count))
                .children(
                    current
                        .as_ref()
                        .map(|step| host_text::element(step).min_w_0().truncate()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(assets::TERMINAL_FONT)
                        .child(clock(*elapsed)),
                );
            (line, sentence, card_help(id, name))
        }
    };
    StatusMark::new(
        SharedString::from(format!("host-status-{id}")),
        sentence.accessible_name(),
        help,
        Role::Status,
        "",
    )
    .ink(palette.warn)
    .described(described(status))
    .truncate()
    .content(line)
    .into_any_element()
}

/// Whether `status`'s hover help is also its accessible description: a running status's is, since the card's words
/// ("Progress of the run on <host>") say what the hover holds, which the status's name does not; a pending status's
/// is not, since its help is its name again, which a screen reader would read twice.
pub(super) fn described(status: &Status) -> bool {
    match status {
        Status::Running { .. } => true,
        Status::Pending(_) => false,
    }
}

/// The running status's hover (SPEC.md, "Topology": "Hovering that status opens the same card as a working host in
/// the attention list"): host `id`'s progress card ([`card`]), made only while it shows (lazy card help, since the
/// card holds the ticker's clock and its running step's pulse) and placed beside the sidebar, as the old card was
/// (`data-tooltip-anchor="session-row"`). Its words, which name the host by `name`, are what the tooltip reports and
/// the status's accessible description ([`status_mark`] describes a running status by them).
///
/// It is the status's one help, and nothing inside the status has its own, so moving the pointer within the status
/// keeps the one card showing rather than making it again, which would restart its pulse. The help is made anew in
/// every render, as every control's is; the layer manager keeps a showing card's view and uses the latest builder
/// only for the next showing.
fn card_help(id: i64, name: &HostText) -> HoverHelp {
    let text = HostSentence::new(SharedString::from(format!("host-progress-card-help-{id}")))
        .text("Progress of the run on ")
        .host(name);
    HoverHelp::card_with(card::WIDTH, move |_, cx| card::view(id, cx).into(), text).beside()
}

// ---- The step list --------------------------------------------------------------------------------------------------

/// How one step looks for its status: its marker (a glyph, or its number when `None`), the marker's ring and ink, the
/// name's and the result word's inks, and the result word (the old `progress_step_status`, which says "Unchanged" and
/// "Warning" where the helm says skipped and degraded).
struct Look {
    glyph: Option<Glyph>,
    ring: Token,
    mark: Token,
    name: Token,
    result: Token,
    word: &'static str,
}

/// A step marker's glyph in place of its number.
#[derive(Clone, Copy)]
enum Glyph {
    Check,
    Text(&'static str),
}

/// The look of a step in `status` (the `.provisioning-step[data-status=...]` rules).
fn look(status: &StepStatus, palette: &Palette) -> Look {
    let quiet = |word| Look {
        glyph: None,
        ring: palette.border_dim,
        mark: palette.fg_dim,
        name: palette.fg_dim,
        result: palette.fg_1,
        word,
    };
    let toned = |glyph, tone: Token, name, word| Look {
        glyph,
        ring: tone,
        mark: tone,
        name,
        result: tone,
        word,
    };
    match status {
        StepStatus::Pending => quiet("Pending"),
        StepStatus::Running => toned(None, palette.warn, palette.warn, "Running"),
        StepStatus::Completed => toned(Some(Glyph::Check), palette.ok, palette.ok, "Completed"),
        StepStatus::Skipped => Look {
            glyph: Some(Glyph::Check),
            result: palette.fg_dim,
            ..quiet("Unchanged")
        },
        StepStatus::Degraded => toned(
            Some(Glyph::Text("!")),
            palette.warn,
            palette.fg_0,
            "Warning",
        ),
        StepStatus::Failed => toned(
            Some(Glyph::Text("×")),
            palette.danger_strong,
            palette.danger_strong,
            "Failed",
        ),
    }
}

/// Host `id`'s retained `run` under its open row (`.provisioning-run` inside `.provisioning-panel`): "Setup
/// provisioning" or "Update provisioning" and the run's state, Collapse at the right when `collapse` is given (the user
/// may fold a running run the row opened for), then every step and the run's own message. The running step's ring is at
/// `pulse`'s opacity.
///
/// Drawn under `host-run-<id>`; each step is a list item named "<step>: <result>" under `host-step-<id>-<n>`, its
/// message under `host-step-<id>-<n>-message`, and the run's message under `host-run-message-<id>`, which is how the
/// panel's tests and the agent bridge read a run.
pub(crate) fn run_section(
    id: i64,
    run: &ProvisioningView,
    collapse: Option<Button>,
    pulse: f32,
    palette: &Palette,
) -> AnyElement {
    let (state, state_ink) = match run.status {
        RunStatus::Running => ("Running", palette.fg_1),
        RunStatus::Completed => ("Completed", palette.fg_1),
        RunStatus::Failed => ("Failed", palette.danger_strong),
    };
    // A run no row draws never gets here ([`shown`]); were one to, it would read as a setup rather than draw nothing.
    let kind = Run::of(run.operation).unwrap_or(Run::Setup);
    let title = kind.title();
    let header = div()
        .id(SharedString::from(format!("host-run-title-{id}")))
        .role(Role::Heading)
        .aria_label(format!("{title} {state}"))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(STEP_GAP))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .child(title),
        )
        .child(div().text_color(theme::color(state_ink)).child(state))
        .children(collapse.map(|button| div().ml_auto().child(button)));
    let count = run.steps.len();
    let steps = run
        .steps
        .iter()
        .enumerate()
        .map(|(n, step)| step_row(Surface::Panel, id, n, step, n + 1 == count, pulse, palette));
    let message = run.message.as_deref().map(|message| {
        div()
            .pt(px(2.))
            .text_color(theme::color(palette.fg_dim))
            .child(
                HostSentence::new(SharedString::from(format!("host-run-message-{id}")))
                    .host(&HostText::from_host(message)),
            )
    });
    div()
        .id(SharedString::from(format!("host-run-{id}")))
        .mt(px(5.))
        .mb(px(1.))
        .ml(px(10.))
        .py(px(3.))
        .px(px(10.))
        .border_l_2()
        .border_color(theme::color(palette.border_nested))
        .text_size(metrics::TEXT_SM)
        .line_height(px(LINE))
        .child(header)
        .child(
            div()
                .id(SharedString::from(format!("host-steps-{id}")))
                .role(Role::List)
                .aria_label(kind.steps())
                .flex()
                .flex_col()
                .mt(px(4.))
                .mb(px(2.))
                .children(steps),
        )
        .children(message)
        .into_any_element()
}

/// Where a step row is drawn, which decides what it shows under its name and the ids it is drawn under. The open row's
/// step list and the progress card draw one step the same way, marker, rail, name and result word; they differ only in
/// the lines under the name and in how the running step's name is set, as the old panel's `.provisioning-step` and the
/// old card's copy of it (`.host-progress-card .provisioning-step`) did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    /// The open row's step list ([`run_section`]): the step's message under its name, whatever its result. Ids start
    /// `host-step-`.
    Panel,
    /// The progress card ([`card`]): the step's explanation under its name, and its message only for a warning or a
    /// failure (SPEC.md: "every step with its status and explanation, ... and any degraded or failed step's reason");
    /// the running step's name set semibold, the old `.host-progress-step-current`. Ids start `host-card-step-`.
    Card,
}

impl Surface {
    /// What a step row's ids start with on this surface: the two surfaces draw the same host's run in one window at
    /// once (the card beside the open row), and tests and the agent bridge tell them apart by these.
    fn step_ids(self) -> &'static str {
        match self {
            Surface::Panel => "host-step",
            Surface::Card => "host-card-step",
        }
    }

    /// The lines this surface draws under `step`'s name, in order: the step's explanation and its message.
    fn lines(self, step: &StepView) -> Vec<&str> {
        match self {
            Surface::Panel => step.message.as_deref().into_iter().collect(),
            Surface::Card => {
                let reason = matches!(step.status, StepStatus::Degraded | StepStatus::Failed)
                    .then_some(step.message.as_deref())
                    .flatten();
                step.explanation
                    .as_deref()
                    .into_iter()
                    .chain(reason)
                    .collect()
            }
        }
    }
}

/// Step `n` of host `id`'s run (`.provisioning-step`) on `surface`: its marker on the rail that runs down to the next
/// step unless it is the `last`, its name and its result word on one line, and the lines the surface shows under the
/// name ([`Surface::lines`]).
fn step_row(
    surface: Surface,
    id: i64,
    n: usize,
    step: &StepView,
    last: bool,
    pulse: f32,
    palette: &Palette,
) -> AnyElement {
    let look = look(&step.status, palette);
    let name = HostText::from_host(step.name.as_str());
    let running = step.status == StepStatus::Running;
    let ids = surface.step_ids();
    // The running step's ring pulses on its own, over a transparent edge, so its number stays solid.
    let ring = running.then(|| {
        div()
            .absolute()
            .top(px(-1.))
            .left(px(-1.))
            .size(px(MARKER))
            .rounded_full()
            .border_1()
            .border_color(theme::color(look.ring))
            .when(pulse < 1., |ring| ring.opacity(pulse))
    });
    let face = match look.glyph {
        Some(Glyph::Check) => svg()
            .path(Icon::Check.path())
            .size(px(9.))
            .text_color(theme::color(look.mark))
            .into_any_element(),
        Some(Glyph::Text(glyph)) => div().child(glyph).into_any_element(),
        None => div().child((n + 1).to_string()).into_any_element(),
    };
    let marker = div()
        .relative()
        .flex_none()
        .size(px(MARKER))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(theme::color(if running {
            theme::TRANSPARENT
        } else {
            look.ring
        }))
        .text_size(px(10.))
        .text_color(theme::color(look.mark))
        .child(face)
        .children(ring);
    let rail = (!last).then(|| {
        div()
            .absolute()
            .left(px(8.))
            .top(px(20.))
            .bottom_0()
            .w(px(1.))
            .bg(theme::color(palette.border_dim))
    });
    let ink = match step.status {
        StepStatus::Failed => palette.danger_strong,
        _ => palette.fg_dim,
    };
    // The panel's one line keeps its old id, `-message`; the card's lines are numbered.
    let lines = surface.lines(step);
    let messages = lines.iter().enumerate().map(|(k, line)| {
        let id = match surface {
            Surface::Panel => format!("{ids}-{id}-{n}-message"),
            Surface::Card => format!("{ids}-{id}-{n}-line-{k}"),
        };
        div()
            .pt(px(2.))
            .pl(px(MARKER_COLUMN + STEP_GAP))
            .text_color(theme::color(ink))
            .child(HostSentence::new(SharedString::from(id)).host(&HostText::from_host(*line)))
    });
    let label = HostSentence::new("host-step-label")
        .host(&name)
        .text(format!(": {}", look.word))
        .accessible_name();
    div()
        .id(SharedString::from(format!("{ids}-{id}-{n}")))
        .role(Role::ListItem)
        .aria_label(label)
        .relative()
        .pt(px(2.))
        .pb(px(9.))
        .children(rail)
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(STEP_GAP))
                .child(div().w(px(MARKER_COLUMN)).flex_none().child(marker))
                .child(
                    host_text::element(&name)
                        .flex_1()
                        .min_w_0()
                        .text_color(theme::color(look.name))
                        .when(running && surface == Surface::Card, |name| {
                            name.font_weight(FontWeight::SEMIBOLD)
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .text_color(theme::color(look.result))
                        .child(look.word),
                ),
        )
        .children(messages)
        .into_any_element()
}

/// Collapse for host `id`'s running run (`.provisioning-run-fold`): `on_fold` folds that run.
pub(crate) fn collapse(id: i64, on_fold: impl Fn(&mut Window, &mut App) + 'static) -> Button {
    Button::new(
        format!("host-run-fold-{id}"),
        "Collapse",
        HoverHelp::tooltip("Collapse this run's steps"),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .compact()
    .on_click(move |_, window, cx| on_fold(window, cx))
}
