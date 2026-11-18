//! What a build of the desktop app without the `agent` feature, which every release and devbuild is, must not carry:
//! the agent build's staged values (`farhelm_gui::agent_startup`, "Staged session values" and "Staged host values"),
//! which the maintainer allowed only on the condition that no release build can contain them. Those sections say how
//! they are fenced; this checks the fence, by reading the binary this package's ordinary build produced.
//!
//! Compiled only without the feature: an agent build is the one build that must carry the seam.

#![cfg(not(feature = "agent"))]

/// The source that defines the variable, read for its value the way `scripts/build-desktop-binary.sh` reads the
/// bridge's `SOCKET_ENV`: a copy of the name here would leave this test passing after a rename, while the new name
/// shipped unchecked.
const AGENT_STARTUP: &str = include_str!("../../gui/farhelm-gui/src/agent_startup.rs");

/// The start of the line in [`AGENT_STARTUP`] that defines the variable; its value follows, up to the closing quote.
const DEFINITION: &str = "pub const SESSION_PINS_ENV: &str = \"";

/// The start of the line that defines the staged file hover action's name, which GPUI registers by that name in every
/// build the action is compiled into.
const ACTION_DEFINITION: &str = "pub const HOVER_STAGED_FILE_ACTION: &str = \"";

/// The value of the one line of [`AGENT_STARTUP`] that starts with `definition`; fails naming it unless exactly one
/// line does.
fn defined(definition: &str) -> &'static str {
    let definitions: Vec<&str> = AGENT_STARTUP
        .lines()
        .filter_map(|line| line.strip_prefix(definition))
        .collect();
    assert_eq!(
        definitions.len(),
        1,
        "agent_startup.rs must define this on one line starting {definition:?}"
    );
    // `split` always yields a first piece.
    definitions[0].split('"').next().unwrap_or_default()
}

/// Whether the file at `path` holds `needle` anywhere, by `grep -F` in the C locale, the search
/// `scripts/build-desktop-binary.sh` runs on a release binary for the bridge. Not a scan in Rust: a debug build of the
/// app is over a gigabyte, mostly debug info, and an unoptimized byte-by-byte scan of it took over a minute.
fn holds(path: &str, needle: &str) -> std::io::Result<bool> {
    let status = std::process::Command::new("grep")
        .args(["-qF", "-e", needle, path])
        .env("LC_ALL", "C")
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(std::io::Error::other(format!(
            "grep failed on {path}: {status}"
        ))),
    }
}

/// A release build must hold no code that rewrites what the helm pushed or what a host answers, so it must not be able
/// to read the staged values at all. The reader names its variable in the binary (in its lookup and in its error
/// message), so a build without the variable's name has no reader; the staged file hover is an action GPUI registers by
/// its name, so a build without that name has no hover. Specified: `agent_startup.rs` defines each name on exactly one
/// line, and the ordinary build of `farhelm-desktop` holds neither value, while it does hold
/// `FARHELM_DESKTOP_STATE_DIR`, which every build reads, so the search is shown to find a name that is there.
#[test]
fn an_ordinary_build_cannot_read_staged_session_values() {
    let name = defined(DEFINITION);
    assert!(
        name.starts_with("FARHELM_"),
        "the premise: {name:?} is a variable name"
    );
    let action = defined(ACTION_DEFINITION);
    assert!(
        action.starts_with("farhelm_agent::"),
        "the premise: {action:?} is an action name"
    );
    let binary = env!("CARGO_BIN_EXE_farhelm-desktop");
    assert!(
        holds(binary, "FARHELM_DESKTOP_STATE_DIR").unwrap(),
        "the premise: the search finds a variable every build reads"
    );
    assert!(
        !holds(binary, name).unwrap(),
        "an ordinary build carries the agent build's reader of {name}"
    );
    assert!(
        !holds(binary, action).unwrap(),
        "an ordinary build carries the agent build's {action}"
    );
}
