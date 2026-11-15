//! Tests of a session row's three marks: the host mark's locality rule and its shape and ink, the agent mark chosen
//! from the launch, and the permission shield's drawing, description and status-line escalation.
//!
//! What a mark shows is decided by pure functions ([`RowMarks::of`] and the functions under it), tested here without
//! a window, because GPUI's driver record of a frame says nothing of colors or of which picture an element painted.
//! The in-window tests check what the record can show: which marks a row draws, their names and hover help, where the
//! title starts, and (by the debug selector each picture carries, [`super::row_marks::picture_selector`]) which
//! picture each one drew. Launches are written in the helm's wire form, as the helm pushes them. The shield's
//! classification itself lives in [`crate::permission`], shared with the approval cards; its tests stay here.

use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::{LaunchHarness, SessionLaunch};
use gpui::{Role, TestAppContext};
use serde_json::{Value, json};

use super::host_rail::tests::{hosts, local, remote};
use super::row::{ITEM_ID_PREFIX, ROW_ID_PREFIX};
use super::row_marks::{
    AGENT_ID_SUFFIX, Description, HOST_ID_SUFFIX, HostMark, Locality, PERMISSION_ID_SUFFIX,
    RowMarks,
};
use super::row_tests::{help_after_crossing, node, texts_within};
use super::tests::{Fixture, collections, listing, open_main, session};
use crate::assets::Icon;
use crate::harness;
use crate::host_text::HostSentence;
use crate::launcher::words;
use crate::marks::HarnessGlyph;
use crate::permission::{Shield, permission};
use crate::test_support::frame;
use crate::theme::{self, Palette};

// ---- Helpers --------------------------------------------------------------------------------------------------------

/// The fleet the host tests use: the local host (registry id 1, the red laptop) and two remotes, one left at the
/// cloud and one with a rocket in teal.
fn fleet() -> farhelm_proto::ui_channel::CollectionState {
    hosts(&[
        local(),
        remote(2, "ops@render-west.example.invalid"),
        remote(3, "deploy@builder-east.example.invalid").mark("rocket", "teal"),
    ])
}

/// `fixture`'s marks over [`fleet`], or with no host list at all (it has not arrived) unless `with_hosts`, selected or
/// not.
fn marks_of(fixture: &Fixture, with_hosts: bool, selected: bool) -> RowMarks {
    let mut snapshot = vec![listing(&[fixture.key], std::slice::from_ref(fixture))];
    if with_hosts {
        snapshot.push(fleet());
    }
    RowMarks::of(
        &fixture.row(),
        &collections(snapshot),
        selected,
        &theme::DARK,
    )
}

/// An agent launch of `harness` with `permissions` (`null` for none) and these start words, as the helm stores one.
fn agent(harness: &str, permissions: Value, start: &[&str]) -> Value {
    let mut start: Vec<Value> = start.iter().map(|word| json!(word)).collect();
    start.push(json!("{farhelm_args}"));
    json!({
        "kind": "agent",
        "selection": {"harness": harness, "permissions": permissions},
        "start": start,
        "resume": null,
    })
}

/// A command launch of `command`, asserted YOLO or not, declaring `agent` (`null` for none).
fn command(command: &str, yolo: bool, agent: Value) -> Value {
    json!({"kind": "command", "command": command, "yolo": yolo, "agent": agent, "resume": null})
}

/// A session from before launch kinds, stored as running `agent_kind`.
fn legacy(invocation: &str, agent_kind: &str) -> Value {
    json!({"kind": "legacy", "invocation": invocation, "agent_kind": agent_kind, "resume_template": null})
}

/// `launch` decoded as the GUI holds it.
fn decoded(launch: Value) -> SessionLaunch {
    serde_json::from_value(launch).unwrap()
}

/// The description's accessible name: the old row's text exactly, which is what a screen reader hears.
fn described(launch: Value, screen_yolo: bool) -> String {
    Description::of(&decoded(launch), screen_yolo)
        .name()
        .to_string()
}

/// The description's hover help as its paragraphs' accessible names, each host value escaped.
fn hovered(launch: Value, screen_yolo: bool) -> Vec<String> {
    Description::of(&decoded(launch), screen_yolo)
        .help("test-help")
        .iter()
        .map(|paragraph| paragraph.accessible_name().to_string())
        .collect()
}

/// The launcher's caveat every agent launch's description ends its permission help with.
const CAVEAT: &str = "Farhelm starts the agent with these choices, but can't guarantee how it behaves. Your own allow \
                      rules, hooks, plugins, always-allow answers, sandbox settings or switching modes inside the \
                      session can loosen it, and agents change between versions. See About permission modes in the \
                      agent documentation.";

// ---- The host mark --------------------------------------------------------------------------------------------------

/// SPEC.md, "Session list": a local row carries its host's mark, a remote row its host's, and "a session whose host
/// cannot yet be placed either way marks neither, rather than guessing". Guessing local from a host name would draw
/// the red laptop on a remote session before the registry answers. Specified: with the registry's local host at id 1,
/// a session on host 1 is local, one on host 2 remote, and one on a host the registry does not list (9) remote too;
/// with no host list at all, every session's locality is unknown and the row draws no host mark.
#[test]
fn a_rows_locality_is_local_remote_or_unplaced() {
    let locality = |host: i64, with_hosts: bool| {
        marks_of(&session("a").host(host), with_hosts, false)
            .host
            .map(|mark| mark.locality)
    };
    assert_eq!(locality(1, true), Some(Locality::Local));
    assert_eq!(locality(2, true), Some(Locality::Remote));
    assert_eq!(locality(9, true), Some(Locality::Remote));
    assert_eq!(locality(1, false), None);
    assert_eq!(locality(2, false), None);
}

/// SPEC.md: a local row defaults to "the red laptop" and a remote row to "the cloud in the ordinary foreground", and
/// either carries its host's chosen icon and color instead; the shape is identity only, so the locality word decides
/// the hover. "On a solid selected row, host, agent and permission marks use the inverse foreground while retaining
/// their shapes." Specified: the untouched local host draws a red laptop in its tint, the untouched remote a cloud in
/// the default color, the remote with a chosen rocket in teal draws that, a host the registry does not list draws the
/// defaults' cloud, and selection keeps the shape and color choice but inks the mark with `--on-fill`.
#[test]
fn the_host_mark_draws_the_hosts_chosen_shape_and_color() {
    let mark = |host: i64, selected: bool| {
        marks_of(&session("a").host(host), true, selected)
            .host
            .unwrap()
    };
    assert_eq!(
        mark(1, false),
        HostMark {
            locality: Locality::Local,
            icon: HostIcon::Laptop,
            color: HostColor::Red,
            inverse: None,
        }
    );
    assert_eq!(
        (mark(2, false).icon, mark(2, false).color),
        (HostIcon::Cloud, HostColor::Default)
    );
    assert_eq!(
        (mark(3, false).icon, mark(3, false).color),
        (HostIcon::Rocket, HostColor::Teal)
    );
    assert_eq!(
        (mark(9, false).icon, mark(9, false).color),
        (HostIcon::Cloud, HostColor::Default)
    );
    assert_eq!(
        mark(3, true),
        HostMark {
            locality: Locality::Remote,
            icon: HostIcon::Rocket,
            color: HostColor::Teal,
            inverse: Some(theme::DARK.on_fill),
        }
    );
}

/// The mark's picture is decorative, so its words are what assistive technology and hover give: SPEC.md's locality
/// explanation, which the old row put on the mark with the hidden word beside it. Specified: a local row's host mark
/// is an image named `Local` that draws the laptop and, hovered, says `Local: on this machine, where the helm runs`;
/// a remote row's is named `Remote`, draws the rocket its host chose and says `Remote: on another machine, reached over
/// SSH`.
#[gpui::test]
fn the_host_mark_names_its_locality_and_explains_it_on_hover(cx: &mut TestAppContext) {
    let rows = [session("a").host(1), session("b").host(3)];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b"], &rows), fleet()]);
    let nodes = frame(cx);
    let local = node(&nodes, "session-row-a-host").unwrap();
    assert_eq!(
        (local.role, local.label.as_deref()),
        (Some(Role::Image), Some("Local"))
    );
    let remote = node(&nodes, "session-row-b-host").unwrap();
    assert_eq!(remote.label.as_deref(), Some("Remote"));
    assert!(cx.debug_bounds("session-row-a-host:laptop").is_some());
    assert!(cx.debug_bounds("session-row-b-host:rocket").is_some());
    assert_eq!(
        help_after_crossing(cx, "session-row-a", "session-row-a-host").as_deref(),
        Some("Local: on this machine, where the helm runs")
    );
    assert_eq!(
        help_after_crossing(cx, "session-row-b", "session-row-b-host").as_deref(),
        Some("Remote: on another machine, reached over SSH")
    );
}

/// The unplaced case drawn: SPEC.md's status and locality marks "occupy aligned columns", so a row whose host cannot be
/// placed keeps its locality slot empty at its width rather than letting the title move left. Specified: with no hosts
/// collection, no host mark is drawn, and the title still starts where the row's last line does, which is indented by
/// both slots and their gaps.
#[gpui::test]
fn an_unplaced_row_draws_no_host_mark_and_keeps_its_slot(cx: &mut TestAppContext) {
    let row = session("a").host(1).title("Unplaced title");
    let (_fake, cx) = open_main(cx, vec![listing(&["a"], &[row])]);
    let nodes = frame(cx);
    assert!(node(&nodes, &format!("session-row-a{HOST_ID_SUFFIX}")).is_none());
    let texts = texts_within(&nodes, &format!("{ITEM_ID_PREFIX}a"));
    let x_of = |wanted: &str| {
        texts
            .iter()
            .find(|text| text.text.as_ref() == wanted)
            .unwrap_or_else(|| panic!("{wanted} was not painted"))
            .bounds
            .origin
            .x
    };
    let (title, last_line) = (x_of("Unplaced title"), x_of("buildbox"));
    assert!(
        (f32::from(title) - f32::from(last_line)).abs() < 0.5,
        "the title starts at {title:?}, the last line at {last_line:?}"
    );
}

// ---- The agent mark -------------------------------------------------------------------------------------------------

/// SPEC.md: the agent mark comes from the launch, never from parsing the command. Every agent type must have its own
/// mark, the one named for it (a swapped arm would mark Pi as OMP). Specified: every `LaunchHarness` maps to the glyph
/// whose token is the harness's own wire name with its underscore dropped (`open_code` is the `opencode` mark).
#[test]
fn every_agent_type_draws_the_mark_named_for_it() {
    for harness in LaunchHarness::ALL {
        let wire = serde_json::to_value(harness).unwrap();
        assert_eq!(
            harness::glyph(*harness).token(),
            wire.as_str().unwrap().replace('_', ""),
            "{harness:?}"
        );
    }
}

/// Specified, per launch kind: an agent launch draws its harness's mark (Claude's here, though its command is a
/// wrapper that names something else); a command launch draws its declared agent's mark, or the Terminal mark with
/// none declared, however much its command looks like an agent; a legacy session draws the mark of the kind the
/// supervisor stored (Claude), or the Terminal mark for a generic one. Each inks with its glyph's identity color
/// (`harness.rs` pins the hues), a mark with none (the Terminal mark here) with the agent track's `--fg-1`, and a
/// selected row's with `--on-fill`.
#[test]
fn the_agent_mark_follows_the_launch_and_inks_with_its_identity() {
    let palette: &Palette = &theme::DARK;
    let cases = [
        (
            agent("claude", json!("guarded"), &["sudo-wrap", "claude"]),
            HarnessGlyph::Claude,
        ),
        (
            command("claude --yolo", true, json!("grok")),
            HarnessGlyph::Grok,
        ),
        (
            command("claude --yolo", true, Value::Null),
            HarnessGlyph::Terminal,
        ),
        (legacy("claude", "claude"), HarnessGlyph::Claude),
        (legacy("claude", "generic"), HarnessGlyph::Terminal),
    ];
    for (launch, glyph) in cases {
        let fixture = session("a").launch(launch.clone());
        assert_eq!(
            marks_of(&fixture, false, false).agent,
            (glyph, harness::ink(glyph, palette).unwrap_or(palette.fg_1)),
            "{launch}"
        );
        assert_eq!(
            marks_of(&fixture, false, true).agent,
            (glyph, palette.on_fill),
            "{launch}"
        );
    }
    let plain = session("a").launch(command("claude --yolo", true, Value::Null));
    assert_eq!(
        marks_of(&plain, false, false).agent,
        (HarnessGlyph::Terminal, palette.fg_1),
        "a mark with no color of its own takes the agent track's foreground"
    );
}

/// Specified: the agent mark draws its glyph's own bundled picture (a Grok command launch draws the Grok file, a
/// legacy generic session the Terminal file), so the drawn picture and the pure decision cannot drift apart.
#[gpui::test]
fn the_agent_mark_draws_its_glyphs_picture(cx: &mut TestAppContext) {
    let rows = [
        session("a").launch(command("grok", false, json!("grok"))),
        session("b").launch(legacy("sh", "generic")),
    ];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b"], &rows)]);
    assert!(
        cx.debug_bounds("session-row-a-agent:icons/harness-grok.svg")
            .is_some()
    );
    assert!(
        cx.debug_bounds("session-row-b-agent:icons/harness-terminal.svg")
            .is_some()
    );
    assert_eq!(harness::icon(HarnessGlyph::Grok), Icon::HarnessGrok);
}

// ---- The permission shield ------------------------------------------------------------------------------------------

/// SPEC.md: "Every session row carries one permission mark", whose meaning is the launch's: an agent launch's effective
/// permission (custom or YOLO is YOLO; none is the agent's own setting), a command's assertion, or nothing for a
/// session from before launch kinds. Specified, case by case: the shield and its ink (`--warn` for YOLO and the
/// question mark, `--ok` for a shield, `--fg-1` for the agent's own setting, `--on-fill` on the selected row), and each
/// shield's picture.
#[test]
fn the_shield_follows_the_launch_and_draws_its_mark() {
    let palette: &Palette = &theme::DARK;
    let cases = [
        (
            agent("codex", json!("yolo"), &["codex", "--yolo"]),
            Shield::Yolo,
        ),
        (agent("claude", json!("custom"), &["claude"]), Shield::Yolo),
        (
            agent("codex", json!("guarded"), &["codex"]),
            Shield::Shielded,
        ),
        (agent("codex", Value::Null, &["codex"]), Shield::OwnSetting),
        (agent("omp", json!("approve"), &["omp"]), Shield::Shielded),
        (
            agent("goose", json!("smart_approve"), &["goose"]),
            Shield::Shielded,
        ),
        (agent("goose", json!("chat"), &["goose"]), Shield::Shielded),
        // An older Pi launch stored no permission, which for Pi has always meant YOLO, not its own setting.
        (agent("pi", Value::Null, &["pi"]), Shield::Yolo),
        (command("codex --yolo", true, json!("codex")), Shield::Yolo),
        (command("codex", false, Value::Null), Shield::Shielded),
        (legacy("codex --yolo", "codex"), Shield::Unknown),
    ];
    for (launch, shield) in cases {
        assert_eq!(
            permission(&decoded(launch.clone()), false),
            shield,
            "{launch}"
        );
        let marks = marks_of(&session("a").launch(launch.clone()), false, false);
        assert_eq!(marks.permission, (shield, shield.ink(palette)), "{launch}");
        let selected = marks_of(&session("a").launch(launch.clone()), false, true);
        assert_eq!(selected.permission, (shield, palette.on_fill), "{launch}");
    }
    assert_eq!(Shield::Yolo.ink(palette), palette.warn);
    assert_eq!(Shield::Unknown.ink(palette), palette.warn);
    assert_eq!(Shield::Shielded.ink(palette), palette.ok);
    assert_eq!(Shield::OwnSetting.ink(palette), palette.fg_1);
    assert_eq!(
        [
            Shield::Yolo,
            Shield::Shielded,
            Shield::OwnSetting,
            Shield::Unknown
        ]
        .map(Shield::icon),
        [
            Icon::PermissionYolo,
            Icon::PermissionShielded,
            Icon::PermissionOwnSetting,
            Icon::PermissionUnknown
        ]
    );
}

/// The description is the only place the shield's meaning is spelled out (several modes share one drawing), and the
/// old specs pinned its wording. Specified, for a command asserted YOLO with a declared agent and for a legacy session,
/// the exact strings the old `terminal.spec.ts` pinned (`<agent or Command> — <assertion> — <command>`), and for a
/// command asserted not YOLO without an agent its counterpart. These are one paragraph, so the hover help is that one
/// paragraph.
#[test]
fn a_command_or_legacy_description_names_the_assertion_and_the_command() {
    let cases = [
        (
            command("codex --yolo", true, json!("codex")),
            "Codex — YOLO, as asserted when the command was launched, by the user or by an agent — not checked by \
             Farhelm — codex --yolo",
        ),
        (
            legacy("codex --yolo", "codex"),
            "Command — Unclassified — a command from before launch kinds, which Farhelm never classified — codex \
             --yolo",
        ),
        (
            command("./run.sh", false, Value::Null),
            "Command — Not YOLO, as asserted when the command was launched, by the user or by an agent — not checked \
             by Farhelm — ./run.sh",
        ),
    ];
    for (launch, text) in cases {
        assert_eq!(described(launch.clone(), false), text);
        assert_eq!(hovered(launch, false), [text]);
    }
}

/// SPEC.md: "Every guarded hover names the agent's mode and meaning, shows the actual stored command under Launched
/// with, and explains that allow rules, hooks, plugins … can loosen it", and the own-setting hover "says Not
/// classified". Specified, exactly, as a screen reader hears it (the old text): a guarded Codex launch, an older Codex
/// launch with no permission (the agent's own setting) and an OMP launch in approve each read `<agent> — <the
/// launcher's help for that permission>` then `Launched with: <stored command>`, the help ending with the launcher's
/// caveat; the guarded help names Codex's mode and its flags.
#[test]
fn an_agent_launch_description_gives_the_mode_its_help_and_the_stored_command() {
    let guarded = described(
        agent(
            "codex",
            json!("guarded"),
            &["codex", "-a", "on-request", "-s", "workspace-write"],
        ),
        false,
    );
    assert_eq!(
        guarded,
        format!(
            "Codex — Guarded · Codex sandboxed auto\nCodex runs within its workspace sandbox and asks before stepping \
             outside it. An approval reviewer may decide requests according to your configuration.\n\nFlags: -a \
             on-request -s workspace-write\n\n{CAVEAT}\n\nLaunched with: codex -a on-request -s workspace-write"
        )
    );
    assert_eq!(
        described(
            agent("codex", Value::Null, &["codex", "--older-flag"]),
            false
        ),
        format!(
            "Codex — Not classified · Agent's own setting\nFarhelm did not force a guarded permission mode for this \
             launch. The agent's configuration decides; hosts that ask before YOLO launches treat it as YOLO.\n\n\
             {CAVEAT}\n\nLaunched with: codex --older-flag"
        )
    );
    assert_eq!(
        described(agent("omp", json!("approve"), &["omp"]), false),
        format!(
            "OMP — Approve: the agent asks before every tool action\n\n{CAVEAT}\n\nLaunched with: omp"
        )
    );
}

/// The hover help is the description's paragraphs, each a host sentence the paragraph tooltip draws with a line
/// between them, as the old app drew help with paragraphs. A sentence lays its parts out as one wrapping row, so a host
/// value after a line break would sit beside the line before it: the stored command was once drawn above its own
/// "Launched with:" label that way. Specified, exactly: a guarded Codex launch's help is four paragraphs (the mode with
/// its heading's line break, Guarded's flags, the caveat, and `Launched with: <command>`), the paragraphs read together
/// with blank lines are the accessible name, and in every paragraph the host value comes after all of the app's words
/// and no line break.
#[test]
fn the_hover_help_is_the_descriptions_paragraphs() {
    let launch = agent(
        "codex",
        json!("guarded"),
        &["codex", "-a", "on-request", "-s", "workspace-write"],
    );
    let paragraphs = hovered(launch.clone(), false);
    assert_eq!(
        paragraphs,
        [
            "Codex — Guarded · Codex sandboxed auto\nCodex runs within its workspace sandbox and asks before stepping \
             outside it. An approval reviewer may decide requests according to your configuration."
                .to_owned(),
            "Flags: -a on-request -s workspace-write".to_owned(),
            CAVEAT.to_owned(),
            "Launched with: codex -a on-request -s workspace-write".to_owned(),
        ]
    );
    assert_eq!(paragraphs.join("\n\n"), described(launch.clone(), false));
    for paragraph in Description::of(&decoded(launch), false).help("help") {
        let parts = format!("{paragraph:?}");
        if let Some(value) = parts.find("Mono(") {
            assert!(!parts[..value].contains("\\n"), "{parts}");
        }
    }
}

/// SPEC.md: "A custom hover says the flags were the user's, that Farhelm does not check them and counts the session
/// as YOLO, and shows both the flags and the full stored command under Launched with." The flags are host values,
/// quoted as shell words. Specified, exactly: a Claude launch with custom flags `--allowedTools` and `Bash(git *)`
/// reads the Custom help, then `Custom permission flags: --allowedTools 'Bash(git *)'`, then the stored command; its
/// hover help ends with those two as paragraphs of their own.
#[test]
fn a_custom_description_shows_the_flags_and_the_stored_command() {
    let mut launch = agent(
        "claude",
        json!("custom"),
        &["claude", "--allowedTools", "Bash(git *)"],
    );
    launch["selection"]["permission_flags"] = json!(["--allowedTools", "Bash(git *)"]);
    let help = words::permission_help(
        LaunchHarness::Claude,
        Some(farhelm_proto::LaunchPermission::Custom),
    );
    assert_eq!(
        described(launch.clone(), false),
        format!(
            "Claude Code — {help}\n\nCustom permission flags: --allowedTools 'Bash(git *)'\n\nLaunched with: claude \
             --allowedTools 'Bash(git *)'"
        )
    );
    assert!(hovered(launch, false).ends_with(&[
        "Custom permission flags: --allowedTools 'Bash(git *)'".to_owned(),
        "Launched with: claude --allowedTools 'Bash(git *)'".to_owned(),
    ]));
}

/// The description quotes host values, which must reach the screen and assistive technology escaped (SPEC_impl.md,
/// "Host-supplied text"); the old `sidebar.spec.ts` case "a bidi override in the invocation basename renders escaped
/// and isolated" pinned the agent mark's text for a command holding a right-to-left override. Specified: that command
/// launch's description is exactly the old pinned text, the override written as its escape, and the command is a host
/// part of its own in the hover help, not the app's words.
#[test]
fn the_description_escapes_the_command_it_quotes() {
    let launch = command(
        "/opt/bin/\u{202e}evil-agent --some-flag",
        false,
        Value::Null,
    );
    assert_eq!(
        described(launch.clone(), false),
        "Command — Not YOLO, as asserted when the command was launched, by the user or by an agent — not checked by \
         Farhelm — /opt/bin/<U+202E>evil-agent --some-flag"
    );
    let help = Description::of(&decoded(launch), false).help("help");
    let plain = HostSentence::new("help-0").text(help[0].accessible_name());
    assert_ne!(help, [plain], "the command is a host part of its own");
}

/// SPEC.md: for Claude, Grok, Muse and Cursor a definite no-checks reading "changes any shield other than YOLO or
/// asserted-YOLO to the YOLO shield", its text names the observed mode while "retaining the launch mode and command",
/// it "never makes the shield safer than its launch choice", legacy sessions and the other agents "are not read", and
/// a later run without evidence shows the launch again. Specified: a guarded Claude launch, an own-setting Cursor
/// launch, a Muse command asserted not YOLO and a Grok one escalate to the YOLO shield, their descriptions leading
/// with the observation in the agent's own words and then `Launched as:` and the launch's own description, unchanged;
/// an asserted-YOLO command keeps the YOLO shield; a Codex launch (no status-line reading) and a legacy session ignore
/// the evidence; and without the evidence every description is the launch's alone.
#[test]
fn status_line_evidence_escalates_the_shield_and_keeps_the_launch_described() {
    let escalating = [
        (
            agent(
                "claude",
                json!("guarded"),
                &["claude", "--permission-mode", "auto"],
            ),
            "bypass permissions on",
        ),
        (
            agent("cursor", Value::Null, &["cursor-agent"]),
            "Run Everything",
        ),
        (command("muse", false, json!("muse")), "YOLO"),
        (command("grok", false, json!("grok")), "always-approve"),
    ];
    for (launch, mode) in escalating {
        assert_ne!(permission(&decoded(launch.clone()), false), Shield::Yolo);
        assert_eq!(
            permission(&decoded(launch.clone()), true),
            Shield::Yolo,
            "{launch}"
        );
        assert_eq!(
            described(launch.clone(), true),
            format!(
                "YOLO observed on the agent's status line: {mode}. Best effort; allow rules, hooks and managed \
                 policy are not visible.\n\nLaunched as: {}",
                described(launch.clone(), false)
            )
        );
        let row = marks_of(
            &session("a").launch(launch.clone()).screen_yolo(),
            false,
            false,
        );
        assert_eq!(row.permission.0, Shield::Yolo);
    }
    let asserted = command("claude --yolo", true, json!("claude"));
    assert_eq!(permission(&decoded(asserted), true), Shield::Yolo);
    for unread in [
        agent("codex", json!("guarded"), &["codex"]),
        legacy("claude --dangerously-skip-permissions", "claude"),
    ] {
        let unescalated = permission(&decoded(unread.clone()), false);
        assert_eq!(
            permission(&decoded(unread.clone()), true),
            unescalated,
            "{unread}"
        );
        assert_eq!(
            described(unread.clone(), true),
            described(unread.clone(), false),
            "{unread}"
        );
    }
}

/// SPEC.md: "Every session row carries one permission mark", beside one agent mark, and the old track was one node for
/// a screen reader, named by its hidden description. Specified: in a fleet of an agent launch, a command launch and a
/// legacy session, each row has exactly one agent-track node, an image named by the description's old text, and no
/// separate node for the shield; the track draws the agent's picture and the shield's (by their debug selectors); and
/// hovering the shield shows the description's hover help. This guards the single node, not a single reading: the
/// frame's record has no field for an accessible description, so a track that also described itself with the same
/// text (which a screen reader would read twice) would still pass.
#[gpui::test]
fn every_row_has_one_agent_track_named_once_by_its_description(cx: &mut TestAppContext) {
    let launches = [
        agent("omp", json!("approve"), &["omp"]),
        command("codex --yolo", true, json!("codex")),
        legacy("codex --yolo", "codex"),
    ];
    let rows = [
        session("a").launch(launches[0].clone()),
        session("b").launch(launches[1].clone()),
        session("c").launch(launches[2].clone()),
    ];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b", "c"], &rows)]);
    let nodes = frame(cx);
    for (key, launch) in ["a", "b", "c"].into_iter().zip(&launches) {
        let row = format!("{ROW_ID_PREFIX}{key}");
        let track = format!("{row}{AGENT_ID_SUFFIX}");
        let shield = format!("{row}{PERMISSION_ID_SUFFIX}");
        let count = |id: &str| {
            nodes
                .iter()
                .filter(|node| node.element_id.as_deref() == Some(id))
                .count()
        };
        assert_eq!(count(&track), 1, "{track}");
        assert_eq!(
            count(&shield),
            0,
            "{shield} is a picture inside the track, not a node"
        );
        let track = node(&nodes, &track).unwrap();
        assert_eq!(track.role, Some(Role::Image));
        assert_eq!(
            track.label.as_deref(),
            Some(described(launch.clone(), false).as_str())
        );
    }
    for picture in [
        "session-row-a-agent:icons/harness-omp.svg",
        "session-row-a-permission:icons/permission-shielded.svg",
        "session-row-b-permission:icons/permission-yolo.svg",
        "session-row-c-permission:icons/permission-unknown.svg",
    ] {
        assert!(cx.debug_bounds(picture).is_some(), "{picture}");
    }
    let shield = cx
        .debug_bounds("session-row-a-permission:icons/permission-shielded.svg")
        .unwrap();
    let help = super::row_tests::help_at(cx, "session-row-a", shield.center());
    assert_eq!(
        help.as_deref(),
        Some(described(launches[0].clone(), false).as_str())
    );
}
