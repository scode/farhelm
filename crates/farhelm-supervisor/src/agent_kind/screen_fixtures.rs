//! Real agent screens captured by `scripts/capture-agent-screens.py` or the
//! manual procedure in `.agents/real-agent-tui.md`, and the
//! tests that hold them to their contract.
//!
//! Each fixture is one visible screen as text, in the form the sampler reads
//! it, that the capture tool saw while it had DRIVEN the agent into a known
//! state, stored as `<harness>/<version>/<expected>-<scenario>.txt` with the
//! terminal's title beside it in a `.title` file. The tool captures in a real
//! terminal host with the environment a supervisor gives an agent (the
//! emulator's `screen_text`); versions captured before November 2026 are a
//! tmux pane's `capture-pane -p`, the old terminals' form, and stay because
//! agents started before their new terminal's environment changed still
//! draw those screens. The expected state in the
//! name is the tool's ground truth, never a reader's verdict, which is what
//! lets these screens act as an oracle for the per-harness screen readers:
//! after a vendor upgrade, re-running the tool and then these tests shows which
//! new screens the readers no longer understand. The last tests paint every
//! screen into the supervisor's terminal emulator, so the readers are held to
//! a new terminal's text as well as to the captured text.
//!
//! Screens are committed to a public repository, so the capture tool scrubs
//! them and this module refuses any that still look identifying — a second,
//! independent check on the one place personal data could enter.

use super::screen_reader::{
    Reading, SAMPLE_TAIL_BYTES, SampleCounts, Screen, ScreenState, reader_for,
};
use farhelm_proto::AgentKind;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, trim_screen_tail};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The states a fixture's file name may promise. `unknown` is a screen the
/// reader should decline to classify (a menu, a transcript viewer), keeping
/// whatever it last concluded.
const EXPECTED_STATES: &[&str] = &["working", "waiting", "idle", "unknown"];

/// Harnesses represented by real or explicitly derived screen fixtures.
const HARNESSES: &[&str] = &["claude", "codex", "muse"];

/// The agent kind for a harness as fixture directories name it, one of
/// [`HARNESSES`].
fn agent_kind(harness: &str) -> AgentKind {
    match harness {
        "claude" => AgentKind::Claude,
        "codex" => AgentKind::Codex,
        "muse" => AgentKind::Muse,
        other => panic!("no agent kind for harness {other:?}"),
    }
}

/// One captured screen with the state the capture tool drove the agent into.
#[derive(Debug)]
struct ScreenFixture {
    /// The harness whose terminal was captured.
    harness: String,
    /// The vendor's version string the screen was captured from.
    version: String,
    /// One of [`EXPECTED_STATES`].
    expected: String,
    /// The rest of the file name, naming what the tool was doing.
    scenario: String,
    /// The raw visible grid.
    screen: String,
    /// The pane title at capture time, without its trailing newline.
    title: String,
    /// Where the screen came from, for failure messages.
    path: PathBuf,
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
fn load_fixtures() -> Vec<ScreenFixture> {
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

/// Older Muse trust-only captures predate its activity reader and stay useful
/// for workspace hints. They promise no working/idle coverage; later versions
/// join the same complete activity contract as Claude and Codex.
fn has_activity_contract(fixture: &ScreenFixture) -> bool {
    fixture.harness != "muse" || fixture.version != "1.4.10-R6427.1"
}

/// What `kind`'s reader concludes from a screen tail and title, asked the way
/// the sampler asks once a run has history and the screen just changed: the
/// one way every test here reads a screen, so that they all hold the readers
/// to the same sampling state.
fn read_as_sampled(kind: AgentKind, text: &str, title: &str) -> Reading {
    let counts = SampleCounts {
        samples: 9,
        unchanged_streak: 0,
    };
    reader_for(kind).read(counts, &Screen { text, title })
}

/// Whether `text` holds something shaped like an email address: a local part
/// and a dotted domain around one `@`, both built from address characters.
///
/// Hand-rolled because the supervisor has no regex dependency and this is its
/// only use; it errs toward flagging, which costs a rename in the capture
/// tool's scrubber rather than a leak.
pub(super) fn contains_email(text: &str) -> bool {
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

/// Whether `text` holds a URL whose host is this machine or a private
/// network: a loopback or unspecified address, `localhost`, an RFC 1918
/// address, or the capture tool's `host` placeholder for the host name.
///
/// An agent shows such a URL when the capture host's configuration routes it
/// through a local proxy (Codex warns that its base URL is overridden), which
/// describes the capture machine's setup. Hand-rolled for the same reason as
/// [`contains_email`]. Userinfo is skipped only inside the authority, so a
/// password cannot hide a local host and a path's `@` cannot change the host.
pub(super) fn contains_local_url(text: &str) -> bool {
    text.match_indices("://").any(|(at, _)| {
        let authority = text[at + 3..]
            .split(|c: char| "/?#".contains(c) || c.is_whitespace())
            .next()
            .unwrap_or("");
        let rest = authority.rsplit('@').next().unwrap_or("");
        let host = match rest.strip_prefix('[') {
            Some(bracketed) => bracketed.split(']').next().unwrap_or(""),
            None => rest
                .split(|c: char| c == ':' || c == '/' || c.is_whitespace() || ",;)".contains(c))
                .next()
                .unwrap_or("")
                .trim_end_matches('.'),
        };
        let host = host.to_ascii_lowercase();
        let octets: Option<Vec<u8>> = host.split('.').map(|part| part.parse().ok()).collect();
        let private_ip = match octets.as_deref() {
            Some([127, ..] | [10, ..] | [0, 0, 0, 0]) => true,
            Some([192, 168, _, _]) => true,
            Some([172, second, _, _]) => (16..=31).contains(second),
            _ => false,
        };
        private_ip || ["localhost", "host", "::1"].contains(&host.as_str())
    })
}

/// The placeholder the capture tool writes for every address it scrubs.
const SCRUBBED_EMAIL: &str = "user@example.com";

/// The capture tool's structural contract holds for every committed screen:
/// names parse to a known state, each has a title sidecar, and every captured
/// activity version of every harness shows the agent working, waiting, and idle.
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
            .filter(|fixture| fixture.harness == *harness && has_activity_contract(fixture))
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

/// No committed screen carries an email address, an absolute home path, or
/// a URL into the capture machine or its private network.
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
            assert!(
                !contains_local_url(text),
                "fixture {} contains a URL into the capture machine or its network",
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

/// The local-URL detector flags URLs into this machine or a private network
/// and leaves public URLs alone.
///
/// Why it matters: a detector that never fires would make the personal-data
/// test pass vacuously, and one that fired on every URL would refuse
/// ordinary agent screens (links to documentation). Userinfo must not hide
/// a local host, nor may an `@` in a public URL's path or query replace it.
#[test]
fn screen_fixtures_local_url_detector_matches_local_hosts_only() {
    for local in [
        "base URL is overridden to http://127.0.0.1:8080/backend-api/codex.",
        "http://localhost:3000",
        "ws://[::1]:9/x",
        "https://192.168.1.4/",
        "http://10.0.0.2:80",
        "http://172.20.1.1/",
        "http://host:8080/",
        "http://user@localhost:8080/",
        "http://user:pass@localhost:8080/",
        "http://user@[::1]:8080/",
        "http://user:pass@[::1]:8080/",
        "http://user@localhost:8080?mode=proxy",
        "http://user:pass@[::1]:8080#proxy",
    ] {
        assert!(contains_local_url(local), "{local}");
    }
    for public in [
        "https://docs.example.com/x",
        "http://172.32.0.1/",
        "https://hostname.example/",
        "http://proxy.invalid/backend-api/codex",
        "https://user:pass@docs.example.com/x",
        "https://docs.example.com/user@localhost",
        "https://docs.example.com/?next=user@localhost",
        "no url at all",
    ] {
        assert!(!contains_local_url(public), "{public}");
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
    for fixture in load_fixtures().into_iter().filter(has_activity_contract) {
        let kind = agent_kind(&fixture.harness);
        let expected = match fixture.expected.as_str() {
            "working" => ScreenState::Working,
            "waiting" => ScreenState::Waiting,
            "idle" => ScreenState::Idle,
            _ => ScreenState::Unknown,
        };
        let text = trim_screen_tail(&fixture.screen, SAMPLE_TAIL_BYTES);
        let reading = read_as_sampled(kind, text, &fixture.title);
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
    let text = trim_screen_tail(&finished, SAMPLE_TAIL_BYTES);
    let reading = read_as_sampled(AgentKind::Claude, text, &fixture.title);
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
        let text = trim_screen_tail(&fixture.screen, SAMPLE_TAIL_BYTES);
        let reading = read_as_sampled(AgentKind::Codex, text, "");
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

// ---- The emulator's repaint of the captured screens ---------------------------
//
// A new terminal's text comes from the supervisor's emulator, not from a
// capture like the ones stored here, so the readers' expectations must
// survive that move too. The fixtures hold captured text, never the bytes the
// agent wrote, so the readers cannot be run on the emulator's version of the
// same session. What can be checked is the next best thing: paint each
// captured screen into an emulator, row by row at an absolute position, and
// run the reader on the emulator's own text. That holds the emulator to
// exactly the characters agents draw (box drawing, block elements, spinners,
// emoji, wide CJK), including whether each row still fits on its row: a row
// the emulator drew wider than the capture's 133 columns would wrap, and its
// text would no longer match.

/// The terminal width the capture tool uses (`scripts/capture-agent-screens.py`,
/// `PANE_COLUMNS`); a screen's height is its number of rows.
const CAPTURE_COLUMNS: u16 = 133;

/// The capture tool's stand-in for the host name an old terminal reports as
/// its title before a program sets one. The repaint sets no title for it, so
/// the emulator's empty title is what reaches the reader, as it would for a
/// new terminal whose program set none.
const SCRUBBED_HOST: &str = "host";

/// The bytes that paint `rows`, each at its own absolute position, and set
/// `title`.
fn repaint(rows: &[&str], title: &str) -> Vec<u8> {
    let mut out = String::new();
    if title != SCRUBBED_HOST && !title.is_empty() {
        out.push_str(&format!("\x1b]2;{title}\x07"));
    }
    for (index, row) in rows.iter().enumerate() {
        out.push_str(&format!("\x1b[{};1H{row}", index + 1));
    }
    out.into_bytes()
}

/// Every captured agent screen, painted into the supervisor's emulator,
/// reads back as the same text, and the emulator's tail and title read as
/// the captured screen does.
///
/// Why: status detection's readers are held to these screens, which change only
/// by re-capture, and a new terminal hands them the emulator's text instead of
/// a capture. A character the emulator draws wide enough to push a row past the
/// capture's width, or a format rule it spells differently, would turn a
/// recognized prompt into an unrecognized one only in new terminals, where
/// nothing else would notice. This replaced a comparison with a live tmux; it
/// needs none, because the captured text itself is the expected value. Spec:
/// for each fixture, the emulator's screen text equals the fixture (the premise
/// that the repaint reproduces the screen, which a row drawn too wide would
/// break), and the fixture's reader, given the emulator's own tail
/// (`Emulator::screen_tail`) and title, reaches the same state and anchoring as
/// on the fixture's text trimmed as the sampler trims it and its title.
#[test]
fn screen_fixtures_read_the_same_from_the_emulator() {
    let mut failures = Vec::new();
    let fixtures = load_fixtures();
    assert!(!fixtures.is_empty(), "no captured screens found");
    for fixture in &fixtures {
        let name = format!(
            "{} {} {}-{}",
            fixture.harness, fixture.version, fixture.expected, fixture.scenario
        );
        let rows: Vec<&str> = fixture.screen.lines().collect();
        let size = Size {
            columns: CAPTURE_COLUMNS,
            lines: u16::try_from(rows.len()).expect("rows"),
        };
        let mut emulator = Emulator::new(size, 0);
        let mut effects = Collected::default();
        emulator.feed(
            &repaint(&rows, &fixture.title),
            Instant::now(),
            FeedMode::Live,
            &mut effects,
        );
        let text = emulator.screen_text();
        if text != fixture.screen {
            failures.push(format!(
                "{name}: the repaint does not reproduce the screen:\n  emulator: {text:?}\n  fixture:  {:?}",
                fixture.screen
            ));
            continue;
        }
        let kind = agent_kind(&fixture.harness);
        let captured = read_as_sampled(
            kind,
            trim_screen_tail(&fixture.screen, SAMPLE_TAIL_BYTES),
            &fixture.title,
        );
        let painted = read_as_sampled(
            kind,
            &emulator.screen_tail(SAMPLE_TAIL_BYTES),
            emulator.title(),
        );
        let (captured, painted) = (
            (captured.state, captured.anchored),
            (painted.state, painted.anchored),
        );
        if captured != painted {
            failures.push(format!(
                "{name}: the reader concludes {captured:?} (state, anchored) from the captured \
                 screen, {painted:?} from the emulator's"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} screens differ:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}
