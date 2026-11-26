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
//! tmux pane's `capture-pane -p`, and stay because agents in a terminal host
//! started before the host environment changed (v0.34 and v0.35) still draw
//! those screens until they are restarted. The expected state in the
//! name is the tool's ground truth, never a reader's verdict, which is what
//! lets these screens act as an oracle for the per-harness screen readers:
//! after a vendor upgrade, re-running the tool and then these tests shows which
//! new screens the readers no longer understand. The last tests paint every
//! screen into the supervisor's terminal emulator, so the readers are held to
//! the emulator's text as well as to the captured text.
//!
//! Screens are committed to a public repository, so the capture tool scrubs
//! them and this module refuses any that still look identifying — a second,
//! independent check on the one place personal data could enter.

use super::screen_reader::{
    Reading, SAMPLE_TAIL_BYTES, SampleCounts, Screen, ScreenState, reader_for,
};
use farhelm_proto::AgentKind;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, trim_screen_tail};
use std::fs::File;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The states a fixture's file name may promise. `unknown` is a screen the
/// reader should decline to classify (a menu, a transcript viewer), keeping
/// whatever it last concluded.
const EXPECTED_STATES: &[&str] = &["working", "waiting", "idle", "unknown"];

/// Harnesses represented by real or explicitly derived screen fixtures.
const HARNESSES: &[&str] = &["claude", "codex", "muse"];
/// These harnesses have anchored Waiting screens without an activity reader
/// for ordinary output. Their fixtures still go through state and repaint checks.
const TRUST_ONLY_HARNESSES: &[&str] = &["antigravity"];

/// The agent kind for a directory in either of the fixture harness lists.
fn agent_kind(harness: &str) -> AgentKind {
    match harness {
        "claude" => AgentKind::Claude,
        "codex" => AgentKind::Codex,
        "muse" => AgentKind::Muse,
        "antigravity" => AgentKind::Antigravity,
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
            HARNESSES.contains(&harness.as_str())
                || TRUST_ONLY_HARNESSES.contains(&harness.as_str()),
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
            fixtures.extend(load_version_fixtures(&harness, &version, &version_dir));
        }
    }
    fixtures
}

/// Take one complete view of a version's fixture pairs. The capture tool locks the version directory while it
/// swaps files, and a left-behind journal means a later capture must finish recovery before these pairs are read.
fn load_version_fixtures(harness: &str, version: &str, version_dir: &Path) -> Vec<ScreenFixture> {
    let directory = File::open(version_dir)
        .unwrap_or_else(|e| panic!("opening {}: {e}", version_dir.display()));
    directory.try_lock_shared().unwrap_or_else(|e| {
        panic!(
            "could not read fixtures while holding the version lock for {}: {e}",
            version_dir.display()
        )
    });
    let mut screens = Vec::new();
    for entry in std::fs::read_dir(version_dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", version_dir.display()))
    {
        let path = entry.expect("directory entry").path();
        assert!(
            path.file_name()
                .is_none_or(|name| name != ".capture-publish"),
            "fixture publication is unfinished in {}. Run the capture tool again to recover it",
            version_dir.display()
        );
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("txt") => screens.push(path),
            Some("title") => assert!(
                path.with_extension("txt").is_file(),
                "fixture title {} has no matching .txt screen",
                path.display()
            ),
            _ => {}
        }
    }
    screens.sort();
    screens
        .iter()
        .map(|path| load_one(harness, version, path))
        .collect()
}

/// The text and title must describe the same captured terminal state, since the screen reader consumes both.
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

/// A crash can leave two individually valid files from different captures. Refuse a version with a journal even
/// when both sides of the pair have plausible contents.
#[test]
#[should_panic(expected = "fixture publication is unfinished")]
fn screen_fixtures_reject_an_unfinished_publication() {
    let version = tempfile::tempdir().expect("temporary fixture version");
    std::fs::write(version.path().join("idle-fresh.txt"), "new screen").unwrap();
    std::fs::write(version.path().join("idle-fresh.title"), "previous title").unwrap();
    std::fs::create_dir(version.path().join(".capture-publish")).unwrap();
    load_version_fixtures("codex", "1.2.3", version.path());
}

/// A stray title used to disappear from the fixture set. Call it out so an incomplete capture cannot look valid.
#[test]
#[should_panic(expected = "has no matching .txt screen")]
fn screen_fixtures_reject_an_orphan_title() {
    let version = tempfile::tempdir().expect("temporary fixture version");
    std::fs::write(version.path().join("idle-fresh.title"), "orphan title").unwrap();
    load_version_fixtures("codex", "1.2.3", version.path());
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
/// address, IPv4 link-local (`169.254.0.0/16`), CGNAT/tailnet (`100.64.0.0/10`),
/// unique-local or link-local IPv6, or the scrubbed name `host`.
///
/// An agent shows such a URL when the capture host's configuration routes it
/// through a local proxy (Codex warns that its base URL is overridden), which
/// describes the capture machine's setup. Hand-rolled for the same reason as
/// [`contains_email`]. A path or a nonempty query or fragment makes the
/// end of a valid authority explicit. A bare URL can end at prose punctuation,
/// so check shorter candidates too. Each candidate can itself carry userinfo.
pub(super) fn contains_local_url(text: &str) -> bool {
    text.match_indices("://").any(|(at, _)| {
        if !ends_with_url_scheme(&text[..at]) {
            return false;
        }
        let tail = &text[at + 3..];
        let authority = tail
            .split(|c: char| "/?#\"<>`".contains(c) || c.is_whitespace())
            .next()
            .unwrap_or("");
        if let Some(local) = local_authority(authority) {
            let suffix = &tail[authority.len()..];
            let explicit_suffix = suffix.starts_with('/')
                || suffix
                    .strip_prefix(['?', '#'])
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|c| !c.is_whitespace() && !"\"'<>`)]}".contains(c));
            if local || explicit_suffix {
                return local;
            }
        }
        authority
            .match_indices([',', ';', ')', '\'', ']', '}'])
            .any(|(end, _)| local_authority(&authority[..end]) == Some(true))
    })
}

/// Locate the scheme at a word boundary, matching the capture scrubber's URL scope in surrounding prose.
fn ends_with_url_scheme(prefix: &str) -> bool {
    prefix
        .char_indices()
        .rev()
        .take_while(|&(_, c)| c.is_ascii_alphanumeric() || "+.-".contains(c))
        .any(|(offset, c)| {
            c.is_ascii_alphabetic()
                && prefix[..offset]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric() && before != '_')
        })
}

/// Classify a complete host and port, or report that the candidate has no usable authority.
fn local_authority(authority: &str) -> Option<bool> {
    let rest = authority
        .trim_end_matches('.')
        .rsplit('@')
        .next()
        .unwrap_or("");
    if let Some(bracketed) = rest.strip_prefix('[') {
        let (literal, port) = bracketed.split_once(']')?;
        let address = literal.split('%').next()?.parse::<Ipv6Addr>().ok()?;
        return local_url_port(port).then(|| {
            address.is_loopback()
                || address.is_unspecified()
                || address.is_unicast_link_local()
                || address.is_unique_local()
                || address.to_ipv4_mapped().is_some_and(is_local_ipv4)
        });
    }
    let end = rest.find(':').unwrap_or(rest.len());
    let (host, port) = rest.split_at(end);
    let host = host.trim_end_matches('.');
    if !local_url_port(port)
        || host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b))
    {
        return None;
    }
    let private_ip = host
        .parse::<Ipv4Addr>()
        .map_or_else(|_| is_legacy_local_ipv4_host(host), is_local_ipv4);
    Some(private_ip || host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("host"))
}

/// Plain and IPv4-mapped URL hosts share the same privacy policy. The CGNAT block also carries tailnet addresses
/// that can identify a peer on the capture machine's network.
fn is_local_ipv4(address: Ipv4Addr) -> bool {
    is_original_local_ipv4(address)
        || address.is_link_local()
        || matches!(address.octets(), [100, 64..=127, ..])
}

/// Keep classifying zero-padded dotted spellings in the original private ranges when the standard parser refuses them.
/// The four octets still have to parse as decimal bytes, as they did before the two new ranges were added.
fn is_legacy_local_ipv4_host(host: &str) -> bool {
    let Ok(octets) = host
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    let Ok(octets) = <[u8; 4]>::try_from(octets) else {
        return false;
    };
    is_original_local_ipv4(Ipv4Addr::from(octets))
}

/// The ranges common to the original canonical classifier and its fallback for padded hosts.
fn is_original_local_ipv4(address: Ipv4Addr) -> bool {
    address.is_loopback() || address.is_unspecified() || address.is_private()
}

/// A port can be absent or empty. A word after the colon can still be a password in a longer candidate.
fn local_url_port(suffix: &str) -> bool {
    suffix.is_empty()
        || suffix
            .strip_prefix(':')
            .is_some_and(|port| port.bytes().all(|b| b.is_ascii_digit()))
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
    for harness in TRUST_ONLY_HARNESSES {
        assert!(
            fixtures.iter().any(|fixture| {
                fixture.harness == *harness
                    && fixture.expected == "waiting"
                    && fixture.scenario == "trust"
            }),
            "no trust fixture captured for {harness}"
        );
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
/// a local host, and the host has to survive adjacent prose with its own `@`.
/// A public link can still mention a local name in its path or query.
/// Boundary hosts keep the private ranges from widening to neighboring networks.
#[test]
fn screen_fixtures_local_url_detector_matches_local_hosts_only() {
    for local in [
        "base URL is overridden to http://127.0.0.1:8080/backend-api/codex.",
        "http://localhost:3000",
        "ws://[::1]:9/x",
        "http://[::]:8080",
        "http://[fd00::1]:8080/backend-api/codex",
        "http://[fe80::1%25eth0]:8080",
        "http://[::ffff:192.168.1.4]:8080",
        "https://192.168.1.4/",
        "https://192.168.001.001/",
        "http://10.0.0.2:80",
        "http://172.20.1.1/",
        "http://169.254.169.254/",
        "http://100.64.0.1/",
        "http://100.127.255.254:8080/",
        "http://[::ffff:169.254.1.1]/",
        "http://[::ffff:100.100.100.100]:8080/",
        "http://host:8080/",
        "http://user@localhost:8080/",
        "http://user:pass@localhost:8080/",
        "http://user@[::1]:8080/",
        "http://user:pass@[::1]:8080/",
        "http://user@localhost:8080?mode=proxy",
        "http://user:pass@[::1]:8080#proxy",
        "http://user:p@ss@[fd00::1]:8080/",
        "http://localhost:8080,user@example.com",
        "http://localhost:8080,user@example.com?",
        "http://localhost:8080,user@example.com#",
        "http://user:p,ass@localhost:8080,user@example.com",
        "http://localhost:123,p@ss@localhost:8080,user@example.com",
        "http://localhost:123,p@ss@localhost:8080/guide",
        "\"http://localhost:8080\"",
        "\"http://user:p@ss@127.0.0.1.:8080\"",
        "\"http://[fd00::1]:8080\"",
    ] {
        assert!(contains_local_url(local), "{local}");
    }
    for public in [
        "https://docs.example.com/x",
        "http://172.32.0.1/",
        "http://100.63.255.255/",
        "http://100.128.0.0/",
        "http://169.253.0.1/",
        "http://169.255.0.1/",
        "http://[::ffff:100.63.255.255]/",
        "http://[::ffff:100.128.0.0]/",
        "http://[::ffff:169.253.0.1]/",
        "http://[::ffff:169.255.0.1]/",
        "http://192.0.2.1/guide",
        "connected to 169.254.169.254 and 100.64.0.1",
        "address ://100.64.0.1",
        "https://hostname.example/",
        "http://proxy.invalid/backend-api/codex",
        "https://user:pass@docs.example.com/x",
        "https://docs.example.com/user@localhost",
        "https://docs.example.com/?next=user@localhost",
        "https://[2001:db8::1]/user@localhost",
        "https://localhost.example.com/help",
        "https://localhost:8080,user@example.com/guide",
        "https://localhost:8080,user@example.com?view=guide",
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
// A live terminal's text comes from the supervisor's emulator, not from a
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

/// The capture tool's stand-in for the host name tmux reported as a pane's
/// title before a program set one (the screens captured before November
/// 2026). The repaint sets no title for it, so the emulator's empty title is
/// what reaches the reader, as it does for any terminal whose program set
/// none.
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
/// by re-capture, and a live terminal hands them the emulator's text instead of
/// a capture. A character the emulator draws wide enough to push a row past the
/// capture's width, or a format rule it spells differently, would turn a
/// recognized prompt into an unrecognized one only in live terminals, where
/// nothing else would notice. The captured text itself is the expected value.
/// Spec:
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
