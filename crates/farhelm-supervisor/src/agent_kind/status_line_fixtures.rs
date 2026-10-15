//! Permission-mode grids captured without model turns on 2026-10-15. These
//! deliberately live apart from activity fixtures: slash-command menus do not
//! establish working/waiting/idle and need no title sidecars or paid turns.

use super::screen_reader::status_line_yolo;
use farhelm_proto::LaunchHarness;
use std::path::Path;

/// Visit every grid, including unreadable dialogs, with its filename's oracle.
///
/// Version and date are in each directory so future vendor drift can be checked
/// against what was actually drawn. Cursor has no definite non-YOLO footer; its
/// absent label is deliberately an unreadable fixture rather than a false reading.
#[test]
fn captured_permission_footers_match_and_ignore_transcript_mode_words() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/status-lines");
    for (agent, harness) in [
        ("claude", LaunchHarness::Claude),
        ("grok", LaunchHarness::Grok),
        ("muse", LaunchHarness::Muse),
        ("cursor", LaunchHarness::Cursor),
    ] {
        let mut count = 0;
        for version in std::fs::read_dir(root.join(agent)).expect("version directories") {
            for file in std::fs::read_dir(version.expect("version").path()).expect("grids") {
                let path = file.expect("fixture").path();
                let name = path.file_name().unwrap().to_str().unwrap();
                let expected = if name.starts_with("yolo-") {
                    Some(true)
                } else if name.starts_with("not-yolo-") {
                    Some(false)
                } else {
                    assert!(name.starts_with("unreadable-"));
                    None
                };
                let grid = std::fs::read_to_string(&path).expect("captured grid");
                // The sampler reads the trimmed 4096-byte tail, not the raw grid.
                let screen =
                    crate::tmux::retain_pane_tail(&grid, super::screen_reader::SAMPLE_TAIL_BYTES);
                assert!(
                    !super::screen_fixtures::contains_email(&screen),
                    "{}",
                    path.display()
                );
                for prefix in ["/home/", "/Users/", "/root/", "/tmp/"] {
                    assert!(!screen.contains(prefix), "{}: {prefix}", path.display());
                }
                assert_eq!(
                    status_line_yolo(harness, &screen),
                    expected,
                    "{}",
                    path.display()
                );
                // A mode word in conversation text cannot override the current
                // footer. Inject it on each of the lines just above the footer,
                // where a loosely anchored reader would pick it up; the top of
                // the grid would let any bottom-anchored reader pass. Moving the
                // footer can make it unreadable, which keeps the last reading,
                // but it must never turn into the opposite answer.
                let lines: Vec<&str> = screen.lines().collect();
                for above in 1..=4.min(lines.len()) {
                    let mut injected = lines.clone();
                    injected.insert(
                        lines.len() - above,
                        "YOLO always-approve Run Everything bypass permissions on",
                    );
                    let reading = status_line_yolo(harness, &injected.join("\n"));
                    assert!(
                        reading == expected || reading.is_none(),
                        "{} with mode words {above} lines up: {reading:?}",
                        path.display()
                    );
                }
                count += 1;
            }
        }
        assert!(
            count >= if agent == "cursor" { 2 } else { 3 },
            "missing {agent} grids"
        );
    }
}

/// Unsupported agents and malformed Unicode screens never invent mode evidence.
#[test]
fn permission_footer_dispatch_declines_unsupported_and_arbitrary_text() {
    for harness in LaunchHarness::ALL {
        for text in [
            "",
            "YOLO",
            "Run Everything",
            "λ\0�",
            "always-approve\n❯ prompt",
        ] {
            assert_eq!(
                status_line_yolo(*harness, text),
                None,
                "{harness:?}: {text:?}"
            );
        }
    }
}
