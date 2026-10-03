//! Real agent screens captured by `scripts/capture-agent-screens.py`, and the
//! tests that hold them to their contract.
//!
//! Each fixture is one visible tmux grid (`capture-pane -p`, the command the
//! sampler runs) that the capture tool saw while it had DRIVEN the agent into
//! a known state, stored as `<harness>/<version>/<expected>-<scenario>.txt`
//! with the pane title beside it in a `.title` file. The expected state in the
//! name is the tool's ground truth, never a reader's verdict, which is what
//! lets these screens act as an oracle for the per-harness screen readers:
//! after a vendor upgrade, re-running the tool and then these tests shows which
//! new screens the readers no longer understand.
//!
//! Screens are committed to a public repository, so the capture tool scrubs
//! them and this module refuses any that still look identifying — a second,
//! independent check on the one place personal data could enter.

use super::screen_reader::{SAMPLE_TAIL_BYTES, SampleCounts, Screen, ScreenState, reader_for};
use farhelm_proto::AgentKind;
use std::path::{Path, PathBuf};

/// The states a fixture's file name may promise. `unknown` is a screen the
/// reader should decline to classify (a menu, a transcript viewer), keeping
/// whatever it last concluded.
const EXPECTED_STATES: &[&str] = &["working", "waiting", "idle", "unknown"];

/// The harnesses the capture tool drives; a directory outside this list is a
/// typo or a stale leftover rather than a new harness nobody wired up.
const HARNESSES: &[&str] = &["claude", "codex"];

/// One captured screen with the state the capture tool drove the agent into.
#[derive(Debug)]
pub(crate) struct ScreenFixture {
    /// `claude` or `codex`.
    pub(crate) harness: String,
    /// The vendor's version string the screen was captured from.
    pub(crate) version: String,
    /// One of [`EXPECTED_STATES`].
    pub(crate) expected: String,
    /// The rest of the file name, naming what the tool was doing.
    pub(crate) scenario: String,
    /// The raw visible grid.
    pub(crate) screen: String,
    /// The pane title at capture time, without its trailing newline.
    pub(crate) title: String,
    /// Where the screen came from, for failure messages.
    pub(crate) path: PathBuf,
}

/// The fixture tree inside this crate.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screens")
}

/// Every fixture, in a stable order, with structural problems reported as
/// panics naming the offending path.
///
/// Stable ordering keeps a failing assertion pointing at the same screen on
/// every run, which matters when a vendor upgrade breaks several at once.
pub(crate) fn load_fixtures() -> Vec<ScreenFixture> {
    let mut fixtures = Vec::new();
    let root = fixtures_root();
    for harness_dir in sorted_dirs(&root) {
        let harness = file_name(&harness_dir);
        assert!(
            HARNESSES.contains(&harness.as_str()),
            "unexpected harness directory {}",
            harness_dir.display()
        );
        for version_dir in sorted_dirs(&harness_dir) {
            let version = file_name(&version_dir);
            assert!(
                version
                    .split('.')
                    .take(3)
                    .all(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit())),
                "version directory {} does not look like a version",
                version_dir.display()
            );
            let mut screens: Vec<PathBuf> = std::fs::read_dir(&version_dir)
                .unwrap_or_else(|e| panic!("reading {}: {e}", version_dir.display()))
                .map(|entry| entry.expect("directory entry").path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
                .collect();
            screens.sort();
            for path in screens {
                fixtures.push(load_one(&harness, &version, &path));
            }
        }
    }
    fixtures
}

fn load_one(harness: &str, version: &str, path: &Path) -> ScreenFixture {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_else(|| panic!("fixture name is not UTF-8: {}", path.display()));
    let (expected, scenario) = stem.split_once('-').unwrap_or_else(|| {
        panic!(
            "fixture {} is not <expected>-<scenario>.txt",
            path.display()
        )
    });
    assert!(
        EXPECTED_STATES.contains(&expected),
        "fixture {} names unknown state {expected:?}; expected one of {EXPECTED_STATES:?}",
        path.display()
    );
    let screen =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let title_path = path.with_extension("title");
    let title = std::fs::read_to_string(&title_path)
        .unwrap_or_else(|e| panic!("fixture {} has no .title sidecar: {e}", path.display()));
    ScreenFixture {
        harness: harness.to_string(),
        version: version.to_string(),
        expected: expected.to_string(),
        scenario: scenario.to_string(),
        screen,
        title: title.trim_end_matches('\n').to_string(),
        path: path.to_path_buf(),
    }
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| panic!("non-UTF-8 path {}", path.display()))
        .to_string()
}

/// Whether `text` holds something shaped like an email address: a local part
/// and a dotted domain around one `@`, both built from address characters.
///
/// Hand-rolled because the supervisor has no regex dependency and this is its
/// only use; it errs toward flagging, which costs a rename in the capture
/// tool's scrubber rather than a leak.
fn contains_email(text: &str) -> bool {
    let is_local = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let is_domain = |c: char| c.is_ascii_alphanumeric() || ".-".contains(c);
    text.match_indices('@').any(|(at, _)| {
        let local = text[..at]
            .chars()
            .rev()
            .take_while(|&c| is_local(c))
            .count();
        let domain: String = text[at + 1..]
            .chars()
            .take_while(|&c| is_domain(c))
            .collect();
        local > 0 && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
    })
}

/// The placeholder the capture tool writes for every address it scrubs.
const SCRUBBED_EMAIL: &str = "user@example.com";

/// The capture tool's structural contract holds for every committed screen:
/// names parse to a known state, each has a title sidecar, and every captured
/// version of both harnesses shows the agent working, waiting, and idle.
///
/// Why it matters: these fixtures are the only thing tying the screen readers
/// to what the vendors really draw. A misnamed file silently drops out of
/// every reader test that iterates them, and a version captured without one
/// of the three live states would let a reader regression for that state pass
/// unnoticed after the next vendor upgrade.
#[test]
fn screen_fixtures_are_well_formed_and_cover_every_live_state() {
    let fixtures = load_fixtures();
    for fixture in &fixtures {
        assert!(
            !fixture.scenario.is_empty() && !fixture.screen.trim().is_empty(),
            "fixture {} is empty or unnamed",
            fixture.path.display()
        );
    }
    for harness in HARNESSES {
        let mut versions: Vec<&str> = fixtures
            .iter()
            .filter(|fixture| fixture.harness == *harness)
            .map(|fixture| fixture.version.as_str())
            .collect();
        versions.dedup();
        assert!(
            !versions.is_empty(),
            "no fixtures captured for {harness}; run scripts/capture-agent-screens.py"
        );
        for version in versions {
            for state in ["working", "waiting", "idle"] {
                assert!(
                    fixtures.iter().any(|fixture| fixture.harness == *harness
                        && fixture.version == version
                        && fixture.expected == state),
                    "{harness} {version} has no `{state}` fixture; re-run the capture tool"
                );
            }
        }
    }
}

/// No committed screen carries an email address or an absolute home path.
///
/// Why it matters: the repository is public, and agent screens routinely
/// print account emails, home directories, and custom status lines. The
/// capture tool scrubs them; this test is the independent backstop that
/// catches a scrubber gap before review has to.
#[test]
fn screen_fixtures_carry_no_personal_data() {
    for fixture in load_fixtures() {
        for text in [&fixture.screen, &fixture.title] {
            let without_placeholder = text.replace(SCRUBBED_EMAIL, "");
            assert!(
                !contains_email(&without_placeholder),
                "fixture {} contains an email address",
                fixture.path.display()
            );
            for prefix in ["/home/", "/Users/", "/root/"] {
                assert!(
                    !text.contains(prefix),
                    "fixture {} contains an absolute home path ({prefix})",
                    fixture.path.display()
                );
            }
        }
    }
}

/// The email detector flags addresses and leaves ordinary `@` uses alone.
///
/// Why it matters: a detector that never fires would make the personal-data
/// test above pass vacuously.
#[test]
fn screen_fixtures_email_detector_matches_addresses_only() {
    assert!(contains_email("contact alice.b@example.org now"));
    assert!(!contains_email("user@host:~/work"));
    assert!(!contains_email("@mention and a@ b"));
}

/// Every captured screen reads, through its harness's reader, as exactly the
/// state the capture tool drove the agent into, from recognized content
/// rather than the change-counting fallback.
///
/// Why it matters: this is the test that notices a vendor UI change. After
/// an agent upgrade the capture tool writes that version's screens, and any
/// screen the rules no longer understand fails here by harness, version,
/// and scenario. Screens are trimmed exactly as the sampler trims them, so
/// a rule that only matches the untrimmed grid cannot pass.
#[test]
fn screen_fixtures_read_as_the_state_they_were_captured_in() {
    let mut failures = Vec::new();
    for fixture in load_fixtures() {
        let kind = match fixture.harness.as_str() {
            "claude" => AgentKind::Claude,
            "codex" => AgentKind::Codex,
            other => panic!("no agent kind for harness {other}"),
        };
        let expected = match fixture.expected.as_str() {
            "working" => ScreenState::Working,
            "waiting" => ScreenState::Waiting,
            "idle" => ScreenState::Idle,
            _ => ScreenState::Unknown,
        };
        let text = crate::tmux::retain_pane_tail(&fixture.screen, SAMPLE_TAIL_BYTES);
        let counts = SampleCounts {
            samples: 9,
            unchanged_streak: 0,
        };
        let reading = reader_for(kind).read(
            counts,
            &Screen {
                text: &text,
                title: &fixture.title,
            },
        );
        if reading.state != expected || !reading.anchored {
            failures.push(format!(
                "{} {} {}-{}: expected {expected:?}, read {:?}{}",
                fixture.harness,
                fixture.version,
                fixture.expected,
                fixture.scenario,
                reading.state,
                if reading.anchored {
                    ""
                } else {
                    " (fallback: no recognized layout)"
                },
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "screens the readers no longer understand:\n{}",
        failures.join("\n")
    );
}

/// The hand-made Claude background-task screen keeps its idle boundary: when
/// the waiting announcement is replaced with Claude's finished-turn line,
/// the same input-box geometry reads anchored idle.
///
/// Why it matters: the two lines occupy the same place above the prompt, so
/// the working match must be specific to an explicit unfinished announcement
/// rather than any decorative line in that part of the screen.
#[test]
fn claude_background_wait_screen_becomes_idle_after_the_turn_finishes() {
    let fixture = load_fixtures()
        .into_iter()
        .find(|fixture| {
            fixture.harness == "claude"
                && fixture.scenario == "derived-background-agents"
                && fixture.expected == "working"
        })
        .expect("the derived Claude background-agent screen must exist");
    let finished = fixture.screen.replace(
        "✻ Waiting for 5 background agents to finish",
        "✻ Cogitated for 26s · done 3:41 PM",
    );
    assert_ne!(
        finished, fixture.screen,
        "fixture premise: waiting line exists"
    );
    let text = crate::tmux::retain_pane_tail(&finished, SAMPLE_TAIL_BYTES);
    let reading = reader_for(AgentKind::Claude).read(
        SampleCounts {
            samples: 9,
            unchanged_streak: 0,
        },
        &Screen {
            text: &text,
            title: &fixture.title,
        },
    );
    assert_eq!(reading.state, ScreenState::Idle);
    assert!(reading.anchored);
}

/// Codex's on-screen `Working (… • esc to interrupt)` widget alone reads
/// the real working screens as working, without the pane title's spinner.
///
/// Why it matters: the title is the Codex reader's main working signal, and
/// the widget is the backstop for when it is missing (a host whose title
/// cannot be read, where the supervisor substitutes an empty one). The backstop's
/// composer geometry once required a single footer row, which no real
/// 0.159.0 screen has, so it never fired and nothing noticed: every fixture
/// carried a title that answered first. Spec: every captured Codex working
/// screen that shows the widget reads working with an empty title, and at
/// least one such screen exists, so this cannot pass vacuously.
#[test]
fn codex_working_widget_alone_reads_real_screens_as_working() {
    let mut checked = 0;
    for fixture in load_fixtures() {
        if fixture.harness != "codex"
            || fixture.expected != "working"
            || !fixture.screen.contains("esc to interrupt")
        {
            continue;
        }
        let text = crate::tmux::retain_pane_tail(&fixture.screen, SAMPLE_TAIL_BYTES);
        let reading = reader_for(AgentKind::Codex).read(
            SampleCounts {
                samples: 9,
                unchanged_streak: 0,
            },
            &Screen {
                text: &text,
                title: "",
            },
        );
        assert_eq!(
            reading.state,
            ScreenState::Working,
            "{} {}-{} with no title",
            fixture.version,
            fixture.expected,
            fixture.scenario
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no captured Codex working screen shows the widget"
    );
}
