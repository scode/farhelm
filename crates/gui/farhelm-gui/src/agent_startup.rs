//! Startup overrides for agent builds: the first window's size and a forced appearance, so a screenshot batch can
//! capture the app light and dark, narrow and wide, without a resize or theme command in the bridge; the start dialogs
//! turned off, so a bridge run's first step acts on the app rather than on a Welcome card its fresh helm has never
//! seen; and a staged run's values (see "Staged session values" and "Staged host values" below), so a docs-media
//! capture shows each session's age and folder, and each staged host's launch history, checkout trash and file checks,
//! as its scenario designs them.
//!
//! NOTE: Only agent builds (the `agent` feature, which optimized builds refuse) read these values. The rest of this
//! module is compiled into every build, so that the bootstrap has one way to read the overrides and ordinary test runs
//! check the parsing and the bootstrap's use of it, but nothing installs overrides outside an agent build unless a test
//! does. The staged values are the exception: their reader, their rewrites and the hover action exist only in agent
//! builds and this crate's tests. These are test hooks, not settings: the real appearance
//! choice is the user's preference, which the helm's preferences hold (the maintainer decided on 2026-11-13 that
//! appearance is a helm preference, not a per-device one), and a forced appearance wins over that stored choice for
//! as long as the run lasts; the window's real size is its saved frame or the first-launch default. Turning the start
//! dialogs off decides the window's start for the run without writing anything: the helm still holds the Welcome card
//! unseen and the announcements pending. Nothing here is persisted, and the app never offers it.
//!
//! The harness (`scripts/gui-agent.py start --window-size WxH --appearance light|dark`) passes the values through the
//! app's environment, the same way it passes the desktop app's other agent-build variables
//! (`crates/farhelm-desktop/src/agent.rs`). It turns the start dialogs off on every run unless given `--start-dialogs`,
//! because every run starts a fresh helm. With every variable unset an agent build starts like any other. A value that
//! does not parse fails startup rather than being ignored, so a batch never captures the default look under a wrong
//! label, and a run never acts on a card it meant to keep closed.

// The bridge's socket is unauthenticated, and on a Mac the local agent sessions run as the same user, so a build that
// contains it lets any of them click Allow on its own approval card (SPEC_impl.md, "How the rules are enforced"). Shipped
// builds and devbuilds are optimized builds (`--release`, or the `dist` profile that inherits from it), so refusing the
// feature wherever debug assertions are off keeps it out of both without having to name each profile. A debug build
// that turns debug assertions off by hand is refused too, which costs nothing: the bridge is only for development runs.
//
// The guard lives here rather than in the crate root because this module is compiled into every build and is the
// bridge's own, in the refused tier of `docs/gpui/foundation-ownership.md`; the crate root is reported wiring, where
// a change that dropped the guard would be caught only by review.
#[cfg(all(feature = "agent", not(debug_assertions)))]
compile_error!(
    "the `agent` feature (the test-automation bridge) is for debug builds only; it must never reach a release or \
     devbuild, whose optimized profiles turn debug assertions off"
);

use std::borrow::Cow;
#[cfg(any(feature = "agent", test))]
use std::collections::HashMap;

#[cfg(any(feature = "agent", test))]
use farhelm_helm_client::PushedEntry;
use farhelm_helm_client::{FileStat, PushedChanges, PushedState};
use farhelm_proto::CheckoutTrashListing;
use farhelm_proto::session_requests::LaunchHistoryReply;
use farhelm_proto::ui_channel::{HostView, SessionRow};
#[cfg(any(feature = "agent", test))]
use farhelm_proto::{
    ArchivedCheckout, DownloadFileInfo, DownloadFileStatus, LaunchSelection,
    session_requests::{FolderHistoryEntry, LaunchHistoryEntry},
    ui_channel::Collection,
};
use gpui::{App, Global, Pixels, Size, px, size};

use crate::theme::Preference;

/// The first window's size, as `WIDTHxHEIGHT` in logical pixels (for example `900x700`).
pub const WINDOW_SIZE_ENV: &str = "FARHELM_DESKTOP_AGENT_WINDOW_SIZE";

/// The appearance the app starts in and keeps for the run, `light` or `dark`, whatever the system's appearance or the
/// helm's stored choice is.
pub const APPEARANCE_ENV: &str = "FARHELM_DESKTOP_AGENT_APPEARANCE";

/// `off` keeps the dialogs a window opens on its own at start and reset (the Welcome card, then the announcement
/// batch) closed for the run; [`start_dialogs_off`] says why. `off` is the only value: unset is the real behavior.
pub const START_DIALOGS_ENV: &str = "FARHELM_DESKTOP_AGENT_START_DIALOGS";

/// What an agent run asked the app to start with. Installed as a global by the bootstrap before anything reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Replaces the first-launch size; still fitted to the display like the default.
    pub window_size: Option<Size<Pixels>>,
    /// Replaces the starting preference (System) for the whole run: neither the system's appearance nor the helm's
    /// stored choice decides the palette.
    pub preference: Option<Preference>,
    /// The start dialogs stay closed for the whole run; read through [`start_dialogs_off`].
    pub start_dialogs_off: bool,
}

impl Global for Overrides {}

/// The overrides the app's environment asks for. A variable that is set but not valid Unicode, or does not parse, is an
/// error that names it. Agent builds only: an ordinary build, test builds included, never reads these variables.
#[cfg(feature = "agent")]
pub fn from_env() -> anyhow::Result<Overrides> {
    parse(
        var(WINDOW_SIZE_ENV)?.as_deref(),
        var(APPEARANCE_ENV)?.as_deref(),
        var(START_DIALOGS_ENV)?.as_deref(),
    )
}

/// Install what the app's environment asks an agent build for: the [`Overrides`] always, and the [`StagedValues`]
/// when [`SESSION_PINS_ENV`] names a file, their ages counted back from now. A pins file that cannot be read or parsed
/// fails startup naming the variable and the file, so a run never captures real ages and scratch folders under a
/// scenario's name. The bootstrap's first step in an agent build, so the theme, the first window and the first
/// snapshot's store all see them.
#[cfg(feature = "agent")]
pub fn install_from_env(cx: &mut App) -> anyhow::Result<()> {
    use anyhow::Context as _;

    cx.set_global(from_env()?);
    let Some(path) = std::env::var_os(SESSION_PINS_ENV).map(std::path::PathBuf::from) else {
        return Ok(());
    };
    let read = || -> anyhow::Result<StagedValues> {
        let anchor = crate::session_list::age::seconds(crate::ticker::wall_now(cx))
            .context("the system clock is before the epoch")?;
        StagedValues::parse(&std::fs::read_to_string(&path)?, anchor)
    };
    let staged = read().with_context(|| format!("{SESSION_PINS_ENV} names {}", path.display()))?;
    cx.set_global(staged);
    crate::demo_video::install(cx);
    // Only a run with staged values has staged files to hover, so only such a run handles the action; in any other
    // agent run the bridge's `action` finds its name (GPUI registers every action type compiled in) and nothing
    // happens.
    cx.on_action(|_: &HoverStagedTerminalFile, cx| {
        if !crate::terminal::hover_staged_file(cx) {
            tracing::warn!(
                "{HOVER_STAGED_FILE_ACTION}: no terminal shows a staged file path to hover"
            );
        }
    });
    Ok(())
}

/// One variable's value, `None` when unset.
#[cfg(feature = "agent")]
fn var(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => anyhow::bail!("{name} is not valid Unicode"),
    }
}

/// Parse the variables' values. Kept apart from [`from_env`] so tests never touch the process environment.
pub fn parse(
    window_size: Option<&str>,
    appearance: Option<&str>,
    start_dialogs: Option<&str>,
) -> anyhow::Result<Overrides> {
    let window_size = window_size
        .map(|text| {
            let parsed = text
                .split_once('x')
                .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
                .filter(|&(w, h)| w > 0 && h > 0);
            match parsed {
                Some((w, h)) => Ok(size(px(f32::from(w)), px(f32::from(h)))),
                None => {
                    anyhow::bail!("{WINDOW_SIZE_ENV} must be WIDTHxHEIGHT in pixels, not {text:?}")
                }
            }
        })
        .transpose()?;
    let preference = appearance
        .map(|text| match text {
            "light" => Ok(Preference::Light),
            "dark" => Ok(Preference::Dark),
            _ => anyhow::bail!("{APPEARANCE_ENV} must be light or dark, not {text:?}"),
        })
        .transpose()?;
    let start_dialogs_off = match start_dialogs {
        None => false,
        Some("off") => true,
        Some(text) => anyhow::bail!("{START_DIALOGS_ENV} must be off or unset, not {text:?}"),
    };
    Ok(Overrides {
        window_size,
        preference,
        start_dialogs_off,
    })
}

/// The installed overrides, or none when nothing installed them: every build but an agent build, and a test that did
/// not install its own. Optional by design, unlike the foundations' globals ([`crate::bootstrap::installed`]).
pub fn current(cx: &App) -> Overrides {
    cx.try_global::<Overrides>().copied().unwrap_or_default()
}

/// Whether an agent run turned the start dialogs off. When true, the window's own start decision (SPEC.md, "Welcome
/// card" and "Announcements": the Welcome card on a helm that has not seen it, then the pending announcement batch)
/// counts as already made for every window start and reset of this run: the caller opens nothing, reads no preference
/// for it and writes nothing, so the card stays unseen, no announcement is recorded shown, and the helm's state is what
/// the real behavior would find. Only that automatic decision may read this; an open the user asks for (Help's
/// Welcome, and its Announcements history once this helm has one) must not consult it. On a run's fresh helm the
/// history stays empty, so Help offers no Announcements item.
///
/// Why it exists: the bridge (`scripts/gui-agent.py start`) gives every run a fresh helm, so without this every run
/// would start under the Welcome card, and its first click or key would act on the card. Nothing can mark the card
/// seen before the window's first snapshot, since the helm answers requests only once it is up. A run passes
/// `--start-dialogs` to see the real start.
///
/// False in every build but an agent build, and in a test that did not install overrides turning it off:
///
/// ```ignore
/// if agent_startup::start_dialogs_off(cx) {
///     return; // the start decision is made: nothing opens, nothing is written
/// }
/// ```
pub fn start_dialogs_off(cx: &App) -> bool {
    current(cx).start_dialogs_off
}

// ---- Staged session values ------------------------------------------------------------------------------------------
//
// A docs-media scenario (`docs/readme-hero/scenario.json5`) designs each session's age and folder, and a staged run
// cannot produce either for real: its sessions were created moments ago, in scratch directories under the run, whose
// path is the kind of machine detail the picture must not show. The browser capture rewrote both in the helm's HTTP
// replies on their way to the page. The desktop app reaches its helm over an in-memory channel, with nothing in between
// to intercept, so the maintainer chose (2026-11-16) a seam in the app instead: an agent build reads the pins file that
// `scripts/gui-agent.py start --scenario` writes, and the store rewrites the named sessions' last-activity stamp and
// folder as it takes in pushed sessions (`docs/readme-hero/SPEC.md`, "What is real and what is not"). "Staged" rather
// than "pinned" in the names here, because pinning a session to the top of the list is a product feature.
//
// Why the store: it is the one copy of pushed state every view reads (the row, the session pane's details bar, the
// summary card, the launcher's default folder), so one rewrite there reaches all of them. The quick switcher reads its
// own list from the helm and applies the same values to that reply before displaying or matching its rows. Nothing
// else of what the helm pushes is rewritten, the order included (the host values below are answers to reads, not pushed
// state): both lists keep the helm's order (`CollectionMeta::order` for the pushed one). The summary card's "Started"
// stays the real creation time, so it reads later than the staged last activity.
//
// Never in a release build. Everything that reads or applies the values is behind `any(feature = "agent", test)`, the
// `agent` feature cannot compile into an optimized build (the guard at the top of this file), and every other build
// gets [`staged_snapshot`], [`staged_changes`] and [`stage_session_rows`] as pass-throughs.
// `crates/farhelm-desktop/tests/release_build.rs` checks an ordinary build of the app for [`SESSION_PINS_ENV`]'s value,
// which it reads from this file.

/// The variable naming the pins file, `{"sessions": [{"title": ..., "age": ..., "cwd": ...}], "hosts": [...]}`: for
/// each session the scenario titles, its age in seconds and the folder its row shows, and for each scenario host, by
/// its alias, what "Staged host values" below substitutes ([`StagedHost`] has the fields).
#[cfg(any(feature = "agent", test))]
pub const SESSION_PINS_ENV: &str = "FARHELM_DESKTOP_AGENT_SESSION_PINS";

/// A staged run's session values, by title, with the anchor their ages count back from. Installed as a global by an
/// agent build launched with [`SESSION_PINS_ENV`]; absent everywhere else.
///
/// Pushes and switcher replies match sessions by their current title, which is all the harness knows when it writes
/// the file. A renamed session loses its values and shows its real scratch folder, a machine path. A Clone keeps its
/// source's title and gets its staged values. Clone and Replace with start from the staged folder, as New session
/// does from the open session's, a folder that may not exist on this machine. Rows the file does not name are kept as
/// received.
///
/// The anchor is when the app read the file, and it stays put: a staged row's age grows from then on like any age, so
/// it reads older by however long the run took between launching the app and taking its picture (seconds for the
/// README hero's run). Re-anchoring on every push would keep the ages exact, but the open session's automatic Mark read
/// records the stamp the window sees, so a stamp that moved forward on every push would make the open row unseen again
/// after each one and the window mark it again, without end.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedValues {
    /// Seconds since the epoch. Every staged age counts back from it, the host values' included.
    anchor: i64,
    by_title: HashMap<String, Staged>,
    /// The staged hosts, by the alias the harness gives each scenario host ("Staged host values").
    by_alias: HashMap<String, StagedHost>,
}

#[cfg(any(feature = "agent", test))]
impl Global for StagedValues {}

/// The pins file as the harness writes it, after its own checks of the scenario. Strict, so a harness and an app that
/// disagree about the shape fail the launch rather than stage nothing: `hosts` is required, an empty list for a run
/// that stages no host.
#[cfg(any(feature = "agent", test))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PinsFile {
    sessions: Vec<Staged>,
    hosts: Vec<StagedHost>,
}

/// One session's entry in the pins file: its age in seconds and its folder as the scenario spells it (`~/src/farhelm`).
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Staged {
    title: String,
    age: u32,
    cwd: String,
}

#[cfg(any(feature = "agent", test))]
impl StagedValues {
    /// Parse a pins file's `text`, its ages counting back from `anchor` (seconds since the epoch). A repeated title
    /// keeps its last entry, and a repeated alias its last host; the harness, the file's only writer, refuses both.
    pub fn parse(text: &str, anchor: i64) -> anyhow::Result<StagedValues> {
        let file: PinsFile = serde_json::from_str(text)?;
        let by_title = file.sessions.into_iter();
        Ok(StagedValues {
            anchor,
            by_title: by_title
                .map(|staged| (staged.title.clone(), staged))
                .collect(),
            by_alias: file
                .hosts
                .into_iter()
                .map(|host| (host.alias.clone(), host))
                .collect(),
        })
    }

    /// The last-activity stamp and folder a session titled `title` is shown with, if the file names it.
    fn stamp(&self, title: &str) -> Option<(i64, &str)> {
        let staged = self.by_title.get(title)?;
        Some((self.anchor - i64::from(staged.age), &staged.cwd))
    }

    /// Give a named session the same staged age and folder whether the window got it through a push or a read.
    fn apply_row(&self, row: &mut SessionRow) {
        if let Some((stamp, cwd)) = self.stamp(&row.info.title) {
            row.info.last_activity_at = stamp;
            cwd.clone_into(&mut row.info.cwd);
        }
    }

    /// Rewrite the sessions among `entries` that the file names; every other entry stays as pushed.
    fn apply(&self, entries: &mut [(String, PushedEntry)]) {
        for (_, entry) in entries {
            if let PushedEntry::Session(row) = entry {
                self.apply_row(row);
            }
        }
    }
}

/// Apply staged values to the window's own copy of a listing read directly from the helm. The helm keeps the real
/// folders it uses to run sessions. Ordinary builds and agent runs without a scenario leave every row as received.
pub(crate) fn stage_session_rows(rows: &mut [SessionRow], cx: &App) {
    #[cfg(any(feature = "agent", test))]
    if let Some(staged) = cx.try_global::<StagedValues>() {
        for row in rows {
            staged.apply_row(row);
        }
    }
    #[cfg(not(any(feature = "agent", test)))]
    let _ = (rows, cx);
}

/// `snapshot` as the store installs it: with the staged sessions rewritten in an agent build that has
/// `StagedValues`, else unchanged and uncopied.
pub(crate) fn staged_snapshot<'a>(snapshot: &'a [PushedState], cx: &App) -> Cow<'a, [PushedState]> {
    #[cfg(any(feature = "agent", test))]
    if let Some(staged) = cx.try_global::<StagedValues>() {
        let mut snapshot = snapshot.to_vec();
        for state in &mut snapshot {
            if state.collection == Collection::Sessions {
                staged.apply(&mut state.entries);
            }
        }
        return Cow::Owned(snapshot);
    }
    #[cfg(not(any(feature = "agent", test)))]
    let _ = cx;
    Cow::Borrowed(snapshot)
}

/// `changes` as the store applies them: with the staged sessions among the upserts rewritten, like
/// [`staged_snapshot`]. Every push of a staged session goes through here, so its row keeps the staged values however
/// often the helm pushes it.
pub(crate) fn staged_changes<'a>(changes: &'a PushedChanges, cx: &App) -> Cow<'a, PushedChanges> {
    #[cfg(any(feature = "agent", test))]
    if changes.collection == Collection::Sessions
        && let Some(staged) = cx.try_global::<StagedValues>()
    {
        let mut changes = changes.clone();
        staged.apply(&mut changes.upsert);
        return Cow::Owned(changes);
    }
    #[cfg(not(any(feature = "agent", test)))]
    let _ = cx;
    Cow::Borrowed(changes)
}

// ---- Staged host values ---------------------------------------------------------------------------------------------
//
// The same seam, extended by the maintainer's decision of 2026-11-18, for three things a host answers that a staged run
// cannot answer honestly. The docs screenshots (`docs/docs-shots/SPEC.md`, "What is real and what is not") need each of
// them, and the browser capture rewrote each in transit:
//
// - The launch history (`GET /api/launch-history`), behind the launcher's recent setups, its recent folders and its
//   search's "Recent setups". The helm records every staged session's launch there, in its scratch directory under the
//   run, a machine path the capture's leak check refuses; and a fresh run has none of the history a page wants to
//   show. So the reply's launches and folders are replaced by the scenario's, for a staged host, as the reply arrives
//   (`launcher::view::history`). Everything downstream is the launcher's own: grouping, ranking, the filter, and the
//   checks a recent row's model and effort must pass against the launch catalog.
// - The checkout trash (`POST /api/checkout-trash/list`). A staged run has no managed checkouts, and real ones would
//   list archive folders under the run. So a staged host's listing is the scenario's, made here instead of read
//   (`trash`), and its deletes are refused before they are sent: the entries name no real folder, and a Delete must
//   never reach a host for them.
// - A terminal's file check (the helm's stat behind the hover on a printed path). The helm resolves a path against the
//   session's real directory, the run's scratch directory, so the answer would be "Not found" or name a machine path.
//   So a staged host's checks are answered here: Ready, with the scenario's full path and size, for a path it names,
//   and Not found for every other one, without asking the host (`terminal::files`). A click on a staged answer
//   downloads nothing. [`HoverStagedTerminalFile`] lets the bridge put the hover on such a path, since a terminal cell
//   is no element the bridge can point at.
//
// Hosts are matched by their alias, which the harness gives every scenario host before any session exists and writes
// into the pins file beside its values: a host the file does not name is left exactly as it is, and a staged host
// renamed by hand loses its staging (its launch history then shows the real scratch folders again). A staged host's
// values replace everything real, also when the scenario lists nothing for it, so a staged session's scratch folder
// never shows. Ages count back from the same anchor as the sessions' ([`StagedValues`]).
//
// Never in a release build, for the same reasons and through the same fence as the session values: everything that
// reads or applies them is behind `any(feature = "agent", test)`, and every other build gets the functions below as
// pass-throughs. `crates/farhelm-desktop/tests/release_build.rs` checks an ordinary build for the hover action's name
// too ([`HOVER_STAGED_FILE_ACTION`]).

/// One staged host's entry in the pins file: the scenario's values for the host the harness aliased `alias`, ages in
/// seconds before the anchor. Each list may be empty, and an empty one still replaces what the host would answer.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedHost {
    alias: String,
    /// The recent setups, in any order (newest first once staged).
    launches: Vec<StagedLaunch>,
    /// The recent folders under the folder field, in any order (newest first once staged).
    folders: Vec<StagedFolder>,
    /// The managed checkouts in the host's trash.
    trash: Vec<StagedArchive>,
    /// The paths a terminal's file check finds on the host.
    files: Vec<StagedFile>,
}

/// One earlier launch: its folder as the scenario spells it (`~/src/farhelm`), which stands for its canonical path too,
/// and its choices in the wire's own shape, so the launcher reads them exactly as it reads a real history's.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedLaunch {
    cwd: String,
    selection: LaunchSelection,
    age: u32,
}

/// One folder a launch started in, spelled as for [`StagedLaunch`].
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedFolder {
    cwd: String,
    age: u32,
}

/// One archived checkout: `checkout` is its name, which sits in `<root>/.farhelm-trash/` as the supervisor's archives
/// do (`ARCHIVE_DIR_NAME` in `crates/farhelm-supervisor/src/working_copies.rs`), without the archive time a real
/// archive's folder name carries: the dialog shows only that folder's parent.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedArchive {
    repository: String,
    checkout: String,
    root: String,
    age: u32,
    bytes: u64,
}

/// One file a terminal names: `path` as the terminal prints it, and the full path and size the check answers with.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedFile {
    path: String,
    shown_path: String,
    size: u64,
}

/// The bridge's way to hover a staged file path in a terminal (`gpui-agent action farhelm_agent::HoverStagedTerminalFile`):
/// the first path the pins file stages for a terminal's host that its screen shows, terminals with focus first, gets
/// the hover a resting pointer gives it, so its check and the hover display follow as they would for the pointer
/// (`terminal::hover_staged_file`). Exists because a printed path is a run of terminal cells, not an element the
/// bridge's `hover` can aim at. Agent builds only, handled only in a run with a pins file ([`install_from_env`]).
///
/// The hover lasts until the terminal sees the pointer again: a move over its grid, or the pointer leaving it. So a
/// capture must find the pointer outside the terminal when the action runs and park it outside again, which a capture
/// that parks on its own search does (the terminal reacts to hover, so no park spot lies on it).
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, Default, PartialEq, gpui::Action)]
#[action(namespace = farhelm_agent)]
pub struct HoverStagedTerminalFile;

/// [`HoverStagedTerminalFile`]'s name as the bridge spells it. Defined on one line, which the release build's check
/// reads (`crates/farhelm-desktop/tests/release_build.rs`).
#[cfg(any(feature = "agent", test))]
pub const HOVER_STAGED_FILE_ACTION: &str = "farhelm_agent::HoverStagedTerminalFile";

#[cfg(any(feature = "agent", test))]
impl StagedValues {
    /// The staged host `host` is, by its alias.
    fn host(&self, host: &HostView) -> Option<&StagedHost> {
        self.by_alias.get(host.alias.as_deref()?)
    }

    /// `reply` with its launches and folders replaced by `staged`'s, as the history of host `id`, newest first; the
    /// checkout configuration's revision stays the helm's, since the launcher's checkout preview follows it.
    fn history(
        &self,
        staged: &StagedHost,
        id: i64,
        reply: LaunchHistoryReply,
    ) -> LaunchHistoryReply {
        let mut launches: Vec<&StagedLaunch> = staged.launches.iter().collect();
        launches.sort_by_key(|launch| launch.age);
        let mut folders: Vec<&StagedFolder> = staged.folders.iter().collect();
        folders.sort_by_key(|folder| folder.age);
        LaunchHistoryReply {
            launches: launches
                .into_iter()
                .map(|launch| LaunchHistoryEntry {
                    host: id,
                    target: None,
                    github_repo: None,
                    canonical_cwd: Some(launch.cwd.clone()),
                    cwd: launch.cwd.clone(),
                    selection: launch.selection.clone(),
                    created_at: self.anchor - i64::from(launch.age),
                    creation_seq: None,
                })
                .collect(),
            folders: folders
                .into_iter()
                .map(|folder| FolderHistoryEntry {
                    host: id,
                    canonical_cwd: folder.cwd.clone(),
                    canonical_proven: true,
                    display_cwd: folder.cwd.clone(),
                    created_at: self.anchor - i64::from(folder.age),
                    creation_seq: None,
                })
                .collect(),
            checkout_config_revision: reply.checkout_config_revision,
        }
    }

    /// `staged`'s trash as a host's listing: nothing being removed, and every entry measured. Ids are the entries'
    /// places in the file, unique within the host as a real listing's are.
    fn trash(&self, staged: &StagedHost) -> CheckoutTrashListing {
        CheckoutTrashListing {
            checkouts: staged
                .trash
                .iter()
                .enumerate()
                .map(|(index, archive)| ArchivedCheckout {
                    id: format!("staged-{index}"),
                    name: archive.checkout.clone(),
                    repository: archive.repository.clone(),
                    path: format!("{}/.farhelm-trash/{}", archive.root, archive.checkout),
                    archived_at: Some(self.anchor - i64::from(archive.age)),
                    bytes: Some(archive.bytes),
                    removing: false,
                    stuck: None,
                })
                .collect(),
            issues: Vec::new(),
            removing: false,
            // An older peer's answer: absence never resolves a notice, so a refused delete's notice stays in view.
            reports_retired: false,
        }
    }

    /// The file check's answer for `path` on staged host `host`: Ready with its staged path and size when `staged`
    /// names it, Not found otherwise. Never offers opening, whatever the helm's list would say for its type.
    fn file(&self, staged: &StagedHost, host: &HostView, path: &str) -> FileStat {
        let info = match staged.files.iter().find(|file| file.path == path) {
            Some(file) => DownloadFileInfo {
                path: file.shown_path.clone(),
                size: Some(file.size),
                status: DownloadFileStatus::Ready,
            },
            None => DownloadFileInfo {
                path: path.to_owned(),
                size: None,
                status: DownloadFileStatus::NotFound,
            },
        };
        FileStat {
            host: host.name.clone(),
            can_open: Some(false),
            info,
        }
    }
}

/// The staged values and the host `id` names in the current UI session's store, when that host is staged.
#[cfg(any(feature = "agent", test))]
fn staged_host_by_id(id: i64, cx: &App) -> Option<(&StagedValues, &StagedHost, HostView)> {
    let staged = cx.try_global::<StagedValues>()?;
    let store = crate::session::current_store(cx)?;
    let view = store
        .read(cx)
        .collections()
        .get::<HostView>(&HostView::key(id))?
        .clone();
    let host = staged.host(&view)?;
    Some((staged, host, view))
}

/// `reply`, the launch history the helm answered for `host`, as the launcher keeps it: with the staged launches and
/// folders in place of the real ones in an agent build whose pins file stages `host`, else unchanged.
pub(crate) fn staged_launch_history(
    reply: LaunchHistoryReply,
    host: Option<&HostView>,
    cx: &App,
) -> LaunchHistoryReply {
    #[cfg(any(feature = "agent", test))]
    if let Some(view) = host
        && let Some(staged) = cx.try_global::<StagedValues>()
        && let Some(host) = staged.host(view)
    {
        return staged.history(host, view.id, reply);
    }
    #[cfg(not(any(feature = "agent", test)))]
    let _ = (host, cx);
    reply
}

/// The trash listing of host `id` in an agent build whose pins file stages it, which the Trash button takes in place of
/// a read; `None` for every other host and in every other build, where the host is read.
pub(crate) fn staged_trash(id: i64, cx: &App) -> Option<CheckoutTrashListing> {
    #[cfg(any(feature = "agent", test))]
    if let Some((staged, host, _)) = staged_host_by_id(id, cx) {
        return Some(staged.trash(host));
    }
    #[cfg(not(any(feature = "agent", test)))]
    let _ = (id, cx);
    None
}

/// The file check's answer for `path` in a terminal of session `session`, when an agent build's pins file stages that
/// session's host; `None` for every other session and in every other build, where the host is asked.
pub(crate) fn staged_file_stat(session: &str, path: &str, cx: &App) -> Option<FileStat> {
    #[cfg(any(feature = "agent", test))]
    {
        let row_host = crate::session::current_store(cx)?
            .read(cx)
            .collections()
            .get::<SessionRow>(session)?
            .host;
        let (staged, host, view) = staged_host_by_id(row_host, cx)?;
        Some(staged.file(host, &view, path))
    }
    #[cfg(not(any(feature = "agent", test)))]
    {
        let _ = (session, path, cx);
        None
    }
}

/// The paths [`HoverStagedTerminalFile`] looks for in a terminal of session `session`, in the pins file's order: the
/// files its host stages, none for an unstaged host.
#[cfg(any(feature = "agent", test))]
pub(crate) fn staged_file_paths(session: &str, cx: &App) -> Vec<String> {
    let Some(store) = crate::session::current_store(cx) else {
        return Vec::new();
    };
    let Some(row_host) = store
        .read(cx)
        .collections()
        .get::<SessionRow>(session)
        .map(|row| row.host)
    else {
        return Vec::new();
    };
    staged_host_by_id(row_host, cx)
        .map(|(_, host, _)| host.files.iter().map(|file| file.path.clone()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gpui::WindowAppearance;
    use gpui_component::{Theme, ThemeMode};

    use super::*;
    use crate::theme;

    /// A batch labels each capture with the theme and size it asked for, so the values must reach the app exactly.
    #[test]
    fn both_values_parse() {
        let overrides = parse(Some("900x700"), Some("dark"), None).unwrap();
        assert_eq!(overrides.window_size, Some(size(px(900.), px(700.))));
        assert_eq!(overrides.preference, Some(Preference::Dark));
        assert_eq!(
            parse(None, Some("light"), None).unwrap().preference,
            Some(Preference::Light)
        );
    }

    /// Every bridge run starts a fresh helm, which would open the Welcome card over the app; `off` must reach the app
    /// as the start dialogs turned off, or a run's first click lands on the card. Specifies that `off` turns them off
    /// and changes nothing else, and that unset leaves them on.
    #[test]
    fn start_dialogs_off_parses() {
        assert_eq!(
            parse(None, None, Some("off")).unwrap(),
            Overrides {
                start_dialogs_off: true,
                ..Overrides::default()
            }
        );
        assert!(
            !parse(Some("900x700"), Some("dark"), None)
                .unwrap()
                .start_dialogs_off
        );
    }

    /// The welcome module reads the decision through the accessor, so it must report what was installed, and report
    /// the real behavior (dialogs on) where nothing was: every build but an agent build.
    #[gpui::test]
    fn the_accessor_reports_the_installed_start_dialogs(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert!(!start_dialogs_off(cx), "nothing installed: dialogs on");
            cx.set_global(Overrides {
                start_dialogs_off: true,
                ..Overrides::default()
            });
            assert!(start_dialogs_off(cx));
            cx.set_global(Overrides::default());
            assert!(!start_dialogs_off(cx));
        });
    }

    /// An agent build launched without the variables (by hand, say) starts exactly like an ordinary build.
    #[test]
    fn nothing_set_overrides_nothing() {
        assert_eq!(parse(None, None, None).unwrap(), Overrides::default());
    }

    /// The override replaces the first-launch size the main window asks for, so a batch's narrow and wide captures
    /// really are those sizes (the test display is larger than both).
    #[gpui::test]
    fn the_main_window_asks_for_the_requested_size(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let narrow = size(px(900.), px(700.));
            cx.set_global(Overrides {
                window_size: Some(narrow),
                ..Overrides::default()
            });
            let options = crate::bootstrap::window_options(cx);
            let bounds = options.window_bounds.unwrap().get_bounds();
            assert_eq!(bounds.size, narrow);
            cx.set_global(Overrides::default());
            let options = crate::bootstrap::window_options(cx);
            let bounds = options.window_bounds.unwrap().get_bounds();
            assert_eq!(bounds.size, crate::bootstrap::DEFAULT_WINDOW_SIZE);
        });
    }

    /// A forced dark appearance wins over a light system, through the bootstrap's own install: a batch's dark captures
    /// must be dark on a machine (or a headless compositor) that reports light. Before this test, dropping the override
    /// at the theme's install left every test passing; only the captures showed it.
    #[gpui::test]
    fn a_forced_dark_appearance_wins_over_a_light_system(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            // The premise: the test platform reports light, so without the override the launch would be light.
            assert_eq!(cx.window_appearance(), WindowAppearance::Light);
            assert!(!theme::resolve(Preference::System, cx.window_appearance()).dark);
            cx.set_global(Overrides {
                preference: Some(Preference::Dark),
                ..Overrides::default()
            });
            crate::bootstrap::install_app_state(cx).unwrap();
            assert!(theme::palette(cx).dark);
            assert_eq!(
                Theme::global(cx).mode,
                ThemeMode::Dark,
                "Kit agrees with the palette"
            );
            // What `open_main_window` does once the window exists and reports its own (light) appearance.
            theme::follow_system(WindowAppearance::Light, cx);
            assert!(
                theme::palette(cx).dark,
                "the window's appearance must not undo it"
            );
        });
    }

    /// The harness writes ages, and the store needs stamps: each age must become a stamp that many seconds before the
    /// anchor, or a row would show some other age than the scenario's. Specified: every entry is found by its title,
    /// with its age counted back from the anchor and its folder as written, a zero age staging the anchor itself, and
    /// a title the file does not name has none.
    #[test]
    fn staged_values_become_stamps_counted_back_from_the_anchor() {
        let text = r#"{"sessions": [
            {"title": "fix flaky attach test", "age": 120, "cwd": "~/src/farhelm"},
            {"title": "release 0.3.1", "age": 86400, "cwd": "~/src/farhelm"},
            {"title": "now", "age": 0, "cwd": "/srv"}
        ], "hosts": []}"#;
        let values = StagedValues::parse(text, 1_800_000_000).unwrap();
        let staged = |title: &str| values.stamp(title).unwrap();
        assert_eq!(values.by_title.len(), 3);
        assert_eq!(
            staged("fix flaky attach test"),
            (1_799_999_880, "~/src/farhelm")
        );
        assert_eq!(staged("release 0.3.1"), (1_799_913_600, "~/src/farhelm"));
        assert_eq!(staged("now"), (1_800_000_000, "/srv"));
        assert_eq!(values.stamp("unnamed"), None);
    }

    /// The harness checks the scenario before it writes the file; the app's own rule is the file's shape, so a harness
    /// and an app that disagree about it fail the launch instead of staging nothing. Specified: an unknown field and a
    /// missing one are refused, in a session's entry and in a host's (each of its lists included, and a choice the
    /// launch wire does not know), and a file without `hosts` is refused, while the entries they spoil are accepted.
    #[test]
    fn a_pins_file_of_another_shape_is_refused() {
        let entry = r#"{"title": "a", "age": 1, "cwd": "~/a"}"#;
        let host = HOST.to_owned();
        let file =
            |entry: &str, host: &str| format!(r#"{{"sessions": [{entry}], "hosts": [{host}]}}"#);
        assert!(
            StagedValues::parse(&file(entry, &host), 1_000).is_ok(),
            "the premise"
        );
        let spoiled_sessions = [
            entry.replace(r#""cwd""#, r#""folder""#),
            entry.replace(r#", "cwd": "~/a""#, ""),
        ];
        for spoiled in spoiled_sessions {
            assert!(
                StagedValues::parse(&file(&spoiled, &host), 1_000).is_err(),
                "{spoiled}"
            );
        }
        let spoiled_hosts = [
            host.replace(r#""alias""#, r#""name""#),
            host.replace(r#""files""#, r#""paths""#),
            host.replace(
                r#""folders": [{"cwd": "~/work/notes", "age": 259200}, {"cwd": "~/src/farhelm", "age": 600}],"#,
                "",
            ),
            host.replace(r#""shown_path""#, r#""full_path""#),
            host.replace(r#""harness": "claude""#, r#""harness": "clod""#),
            host.replace(r#""bytes": 48200000"#, r#""bytes": -1"#),
        ];
        for spoiled in spoiled_hosts {
            assert!(
                StagedValues::parse(&file(entry, &spoiled), 1_000).is_err(),
                "{spoiled}"
            );
        }
        let without_hosts = format!(r#"{{"sessions": [{entry}]}}"#);
        assert!(StagedValues::parse(&without_hosts, 1_000).is_err());
    }

    /// One staged host as the harness writes it, aliased "this machine": two launches, two folders, a trash entry
    /// and a file. The trash entry is spelled out so a test can spoil one of its fields.
    const HOST: &str = r#"{"alias": "this machine",
        "launches": [
            {"cwd": "~/src/api", "selection": {"harness": "claude", "model": null, "effort": null,
                                               "permissions": null}, "age": 7200},
            {"cwd": "~/src/farhelm", "selection": {"harness": "codex", "model": null, "effort": "high",
                                                   "permissions": "yolo", "workspace_trust": true}, "age": 600}
        ],
        "folders": [{"cwd": "~/work/notes", "age": 259200}, {"cwd": "~/src/farhelm", "age": 600}],
        "files": [{"path": "e2e/attach.spec.ts", "shown_path": "/Users/you/src/farhelm/e2e/attach.spec.ts",
                   "size": 6412}],
        "trash": [{"repository": "acme/web", "checkout": "web-fix-login", "root": "/Users/you/work",
                   "age": 5400, "bytes": 48200000}]}"#;

    /// `view` with the alias the harness gives a scenario host.
    fn aliased(mut view: HostView, alias: &str) -> HostView {
        view.alias = Some(alias.to_owned());
        view.name = alias.to_owned();
        view
    }

    /// Why this matters: the docs screenshots of the launcher, the trash and a file hover must show the scenario's
    /// values and never the real ones, the run's scratch folders among them
    /// (`docs/docs-shots/SPEC.md`, "What is real and what is not"). Specified for a host matched by its alias: the history's launches and folders are the
    /// staged ones only, newest first, on that host's id, each folder its own canonical path, stamped back from the
    /// anchor, with the choices as written and the helm's checkout revision kept; the trash lists each entry under
    /// `<root>/.farhelm-trash/`, with a unique id, its archive time and size, nothing removing; a staged path's check is
    /// Ready with its full path and size under the host's display name and never offers opening, and any other path
    /// is Not found as asked. A host with no alias, or another alias, is not staged.
    #[test]
    fn a_staged_host_answers_with_the_scenarios_values() {
        let text = format!(r#"{{"sessions": [], "hosts": [{HOST}]}}"#);
        let values = StagedValues::parse(&text, 1_800_000_000).unwrap();
        let view = aliased(
            crate::hosts::tests::host(7, "local", crate::hosts::tests::connected()),
            "this machine",
        );
        let staged = values.host(&view).expect("matched by its alias");
        let mut other = view.clone();
        other.alias = Some("build-box".into());
        assert!(values.host(&other).is_none());
        other.alias = None;
        assert!(values.host(&other).is_none());

        let real = LaunchHistoryReply {
            launches: Vec::new(),
            folders: Vec::new(),
            checkout_config_revision: 41,
        };
        let history = values.history(staged, view.id, real);
        assert_eq!(history.checkout_config_revision, 41);
        let launches: Vec<_> = history
            .launches
            .iter()
            .map(|entry| {
                (
                    entry.host,
                    entry.cwd.as_str(),
                    entry.canonical_cwd.as_deref(),
                    entry.created_at,
                )
            })
            .collect();
        assert_eq!(
            launches,
            [
                (7, "~/src/farhelm", Some("~/src/farhelm"), 1_799_999_400),
                (7, "~/src/api", Some("~/src/api"), 1_799_992_800),
            ],
            "newest first"
        );
        let codex = &history.launches[0].selection;
        assert_eq!(
            (
                codex.harness,
                codex.effort,
                codex.permissions,
                codex.workspace_trust
            ),
            (
                farhelm_proto::LaunchHarness::Codex,
                Some(farhelm_proto::LaunchEffort::High),
                Some(farhelm_proto::LaunchPermission::Yolo),
                Some(true)
            )
        );
        let folders: Vec<_> = history
            .folders
            .iter()
            .map(|folder| {
                (
                    folder.host,
                    folder.display_cwd.as_str(),
                    folder.canonical_cwd.as_str(),
                    folder.created_at,
                )
            })
            .collect();
        assert_eq!(
            folders,
            [
                (7, "~/src/farhelm", "~/src/farhelm", 1_799_999_400),
                (7, "~/work/notes", "~/work/notes", 1_799_740_800),
            ]
        );

        let trash = values.trash(staged);
        assert!(!trash.removing && trash.issues.is_empty());
        assert_eq!(
            trash.checkouts,
            [ArchivedCheckout {
                id: "staged-0".into(),
                name: "web-fix-login".into(),
                repository: "acme/web".into(),
                path: "/Users/you/work/.farhelm-trash/web-fix-login".into(),
                archived_at: Some(1_799_994_600),
                bytes: Some(48_200_000),
                removing: false,
                stuck: None,
            }]
        );

        let ready = values.file(staged, &view, "e2e/attach.spec.ts");
        assert_eq!(
            ready,
            FileStat {
                host: "this machine".into(),
                can_open: Some(false),
                info: DownloadFileInfo {
                    path: "/Users/you/src/farhelm/e2e/attach.spec.ts".into(),
                    size: Some(6412),
                    status: DownloadFileStatus::Ready,
                },
            }
        );
        let missing = values.file(staged, &view, "src/main.rs");
        assert_eq!(
            (missing.info.path.as_str(), missing.info.status),
            ("src/main.rs", DownloadFileStatus::NotFound)
        );
    }

    /// Why this matters: the launcher hands every history answer through here, staged run or not, so it must change
    /// nothing but a staged host's answer. Specified: without staged values, and for a host the file does not name,
    /// the answer comes back as the helm gave it; for a staged host its launches are the staged ones.
    #[gpui::test]
    fn only_a_staged_hosts_history_is_replaced(cx: &mut gpui::TestAppContext) {
        let view = aliased(
            crate::hosts::tests::host(3, "ssh", crate::hosts::tests::connected()),
            "this machine",
        );
        let real = LaunchHistoryReply {
            launches: vec![LaunchHistoryEntry {
                host: 3,
                target: None,
                github_repo: None,
                canonical_cwd: Some("/tmp/run/work/0".into()),
                cwd: "/tmp/run/work/0".into(),
                selection: LaunchSelection {
                    harness: farhelm_proto::LaunchHarness::Claude,
                    model: None,
                    effort: None,
                    permissions: None,
                    permission_flags: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                },
                created_at: 5,
                creation_seq: None,
            }],
            folders: Vec::new(),
            checkout_config_revision: 2,
        };
        cx.update(|cx| {
            assert_eq!(
                staged_launch_history(real.clone(), Some(&view), cx),
                real,
                "nothing staged"
            );
            let text = format!(r#"{{"sessions": [], "hosts": [{HOST}]}}"#);
            cx.set_global(StagedValues::parse(&text, 1_000_000).unwrap());
            let other = aliased(view.clone(), "gpu-1");
            assert_eq!(staged_launch_history(real.clone(), Some(&other), cx), real);
            assert_eq!(staged_launch_history(real.clone(), None, cx), real);
            let staged = staged_launch_history(real.clone(), Some(&view), cx);
            assert_eq!(
                staged
                    .launches
                    .iter()
                    .map(|entry| entry.cwd.as_str())
                    .collect::<Vec<_>>(),
                ["~/src/farhelm", "~/src/api"]
            );
        });
    }

    /// Why this matters: the docs capture dispatches the hover by name through the bridge, and the release build's
    /// check looks for that name in the binary, so the name both use must be the one GPUI registers. Specified: the
    /// constant is the action type's name, and GPUI builds the action from it with no data.
    #[gpui::test]
    fn the_hover_actions_name_is_the_one_gpui_registers(cx: &mut gpui::TestAppContext) {
        use gpui::Action as _;
        assert_eq!(
            HoverStagedTerminalFile::name_for_type(),
            HOVER_STAGED_FILE_ACTION
        );
        cx.update(|cx| {
            let built = cx.build_action(HOVER_STAGED_FILE_ACTION, None).unwrap();
            assert!(built.partial_eq(&HoverStagedTerminalFile));
        });
    }

    /// A typo must fail the launch: ignoring it would capture the default look under the label the batch asked for, or
    /// leave the Welcome card over the app a run meant to drive.
    #[test]
    fn a_malformed_value_is_an_error_naming_its_variable() {
        for bad in ["900", "900x", "x700", "0x700", "900x-1", "900X700", "wide"] {
            let error = parse(Some(bad), None, None).unwrap_err().to_string();
            assert!(error.contains(WINDOW_SIZE_ENV), "{bad}: {error}");
        }
        for bad in ["system", "Dark", ""] {
            let error = parse(None, Some(bad), None).unwrap_err().to_string();
            assert!(error.contains(APPEARANCE_ENV), "{bad}: {error}");
        }
        for bad in ["on", "Off", "false", "0", ""] {
            let error = parse(None, None, Some(bad)).unwrap_err().to_string();
            assert!(error.contains(START_DIALOGS_ENV), "{bad}: {error}");
        }
    }
}
