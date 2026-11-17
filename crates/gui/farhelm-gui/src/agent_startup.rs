//! Startup overrides for agent builds: the first window's size and a forced appearance, so a screenshot batch can
//! capture the app light and dark, narrow and wide, without a resize or theme command in the bridge; the start dialogs
//! turned off, so a bridge run's first step acts on the app rather than on a Welcome card its fresh helm has never
//! seen; and a staged run's session values (see "Staged session values" below), so a docs-media capture shows each
//! session's age and folder as its scenario designs them.
//!
//! NOTE: Only agent builds (the `agent` feature, which optimized builds refuse) read these values. The rest of this
//! module is compiled into every build, so that the bootstrap has one way to read the overrides and ordinary test runs
//! check the parsing and the bootstrap's use of it, but nothing installs overrides outside an agent build unless a test
//! does. The staged session values are the exception: their reader and their rewrite exist only in agent builds and
//! this crate's tests. These are test hooks, not settings: the real appearance
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
use farhelm_helm_client::{PushedChanges, PushedState};
#[cfg(any(feature = "agent", test))]
use farhelm_proto::ui_channel::Collection;
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
// summary card, the quick switcher, the launcher's default folder), so one rewrite there reaches all of them, as the
// old rewrite of both the listing and the detail reply did. Nothing else is rewritten, the order included: the list's
// order is the helm's (`CollectionMeta::order`). The summary card's "Started" stays the real creation time, so it reads
// later than the staged last activity.
//
// Never in a release build. Everything that reads or applies the values is behind `any(feature = "agent", test)`, the
// `agent` feature cannot compile into an optimized build (the guard at the top of this file), and every other build
// gets [`staged_snapshot`] and [`staged_changes`] as pass-throughs. `crates/farhelm-desktop/tests/release_build.rs`
// checks an ordinary build of the app for [`SESSION_PINS_ENV`]'s value, which it reads from this file.

/// The variable naming the pins file, `{"sessions": [{"title": ..., "age": ..., "cwd": ...}]}`: for each session the
/// scenario titles, its age in seconds and the folder its row shows.
#[cfg(any(feature = "agent", test))]
pub const SESSION_PINS_ENV: &str = "FARHELM_DESKTOP_AGENT_SESSION_PINS";

/// A staged run's session values, by title, with the anchor their ages count back from. Installed as a global by an
/// agent build launched with [`SESSION_PINS_ENV`]; absent everywhere else.
///
/// Sessions are matched on every push by their current title, because that is all the harness knows when it writes the
/// file, before any session exists (the old capture matched by title too). So a renamed session loses its values and
/// shows its real scratch folder, a machine path; a Clone, which keeps its source's title, gets them; and Clone and
/// Replace with start from the staged folder, as New session does from the open session's, a folder that does not
/// exist on this machine. A session the file does not name is left exactly as pushed.
///
/// The anchor is when the app read the file, and it stays put: a staged row's age grows from then on like any age, so
/// it reads older by however long the run took between launching the app and taking its picture (seconds for the
/// README hero's run). Re-anchoring on every push would keep the ages exact, but the open session's automatic Mark read
/// records the stamp the window sees, so a stamp that moved forward on every push would make the open row unseen again
/// after each one and the window mark it again, without end.
#[cfg(any(feature = "agent", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedValues {
    /// Seconds since the epoch.
    anchor: i64,
    by_title: HashMap<String, Staged>,
}

#[cfg(any(feature = "agent", test))]
impl Global for StagedValues {}

/// The pins file as the harness writes it, after its own checks of the scenario. Strict, so a harness and an app that
/// disagree about the shape fail the launch rather than stage nothing.
#[cfg(any(feature = "agent", test))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PinsFile {
    sessions: Vec<Staged>,
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
    /// keeps its last entry; the harness, the file's only writer, refuses one.
    pub fn parse(text: &str, anchor: i64) -> anyhow::Result<StagedValues> {
        let file: PinsFile = serde_json::from_str(text)?;
        let by_title = file.sessions.into_iter();
        Ok(StagedValues {
            anchor,
            by_title: by_title
                .map(|staged| (staged.title.clone(), staged))
                .collect(),
        })
    }

    /// The last-activity stamp and folder a session titled `title` is shown with, if the file names it.
    fn stamp(&self, title: &str) -> Option<(i64, &str)> {
        let staged = self.by_title.get(title)?;
        Some((self.anchor - i64::from(staged.age), &staged.cwd))
    }

    /// Rewrite the sessions among `entries` that the file names; every other entry stays as pushed.
    fn apply(&self, entries: &mut [(String, PushedEntry)]) {
        for (_, entry) in entries {
            if let PushedEntry::Session(row) = entry
                && let Some((stamp, cwd)) = self.stamp(&row.info.title)
            {
                row.info.last_activity_at = stamp;
                cwd.clone_into(&mut row.info.cwd);
            }
        }
    }
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
        ]}"#;
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
    /// missing one are refused, while the entry they spoil is accepted.
    #[test]
    fn a_pins_file_of_another_shape_is_refused() {
        let entry = r#"{"title": "a", "age": 1, "cwd": "~/a"}"#;
        let file = |entry: &str| format!(r#"{{"sessions": [{entry}]}}"#);
        assert!(
            StagedValues::parse(&file(entry), 1_000).is_ok(),
            "the premise"
        );
        for spoiled in [
            entry.replace(r#""cwd""#, r#""folder""#),
            entry.replace(r#", "cwd": "~/a""#, ""),
        ] {
            assert!(
                StagedValues::parse(&file(&spoiled), 1_000).is_err(),
                "{spoiled}"
            );
        }
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
