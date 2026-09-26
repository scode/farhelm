//! `farhelm-fixtures`: the stand-in programs Farhelm's tests run, kept out
//! of the product binary.
//!
//! The e2e tests and the browser suite need an "agent" that behaves
//! deterministically (prompts, colors, terminal modes, conversation records,
//! floods), and a sweep that reaps the state directories dead test runs
//! leave under `/tmp`. Both used to be hidden `farhelm internal`
//! subcommands, which shipped several thousand lines of test fixtures in
//! every release and made the product binary depend on `farhelm-teststate`.
//! This package is never shipped (`dist = false`).
//!
//! The e2e tests find this binary next to the `farhelm` binary they test,
//! in the same target directory; `.config/nextest.toml` builds it before any
//! e2e test runs. The browser suite's `e2e/start-stack.sh` finds it the same
//! way after `cargo build`, which builds it as a default member.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod fake_agent;

#[derive(Parser)]
#[command(
    name = "farhelm-fixtures",
    about = "Test fixture programs for Farhelm's test suites"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// A scripted TUI standing in for real agents in tests: prompts,
    /// echoes, colors, terminal modes, and raw-byte output — deterministic
    /// and free of vendor auth (PLAN_M1.md's test harness).
    FakeAgent {
        /// Behavior script.
        #[arg(long, value_enum, default_value_t = fake_agent::Script::Basic)]
        script: fake_agent::Script,
        /// Root the record-writing scripts hang their `.claude`/`.codex`
        /// trees off, mirroring the supervisor's own injectable agent home
        /// (PLAN_M3.md item 8). A flag rather than `$HOME` because the
        /// tests that use it must not mutate the test process's
        /// environment, and because concurrent harnesses each need their
        /// own tree. Ignored by every other script.
        #[arg(long)]
        record_home: Option<PathBuf>,
        /// Bracket each chunk of [`fake_agent::Script::FloodRegion`]'s
        /// scrolled records in DEC private mode 2026's synchronized-output
        /// sequences. Ignored by every other script, the same tolerance
        /// `record_home` gets.
        #[arg(long)]
        sync_output: bool,
        /// The transcript [`fake_agent::Script::Replay`] prints before it
        /// settles (see `docs/readme-hero/SPEC.md`). Ignored by every other
        /// script.
        #[arg(long)]
        transcript: Option<PathBuf>,
        /// The shape [`fake_agent::Script::Replay`] holds after the
        /// transcript; each one is chosen for the status the supervisor
        /// classifies it as. Ignored by every other script.
        #[arg(long, value_enum)]
        then: Option<fake_agent::ReplayThen>,
        /// The status `--then exit` exits with.
        #[arg(long, default_value_t = 0)]
        exit_code: i32,
        /// Whatever the supervisor appends for the real vendor after the
        /// fixture's own flags — the per-launch hook flags, or anything a
        /// test's resume template places there. Every script tolerates the
        /// tail; only the record-writing scripts (`claude-record`,
        /// `codex-record`) print the process argv in their `FAKE-AGENT
        /// ARGV:` marker, which is where a test asserts on injection
        /// without the fixture understanding the injected flags.
        ///
        /// The `agent-relay` script is the one that reads the injected tail
        /// rather than merely tolerating it — but it reads
        /// `std::env::args()` directly rather than this field, on purpose:
        /// what it is standing in for is a VENDOR parsing the argv the
        /// supervisor built, so it should see exactly that rather than a
        /// re-parse of it. What this field still does for it is the same
        /// thing it does for every other script — keep clap from rejecting
        /// the whole command line before `run` is ever reached.
        ///
        /// The conversation-capture and restart integration fixtures
        /// (`conversation_identity_capture.rs`, `restart_with_resume.rs`)
        /// ARE this binary, symlinked as `claude` or `codex` and invoked as
        /// `farhelm-fixtures fake-agent`. A supervisor that appends real-vendor
        /// flags to that argv the way it does for the genuine CLI —
        /// `--settings <json>` for Claude-kind sessions, or
        /// `--dangerously-bypass-hook-trust -c ... -c ...` for Codex-kind
        /// ones — would otherwise hand clap flags it has no arm for, and
        /// clap rejects the whole argv with a usage error, killing the
        /// fixture before a single line of script output is produced.
        /// `trailing_var_arg` plus `allow_hyphen_values` make capture begin
        /// at the first token clap does not recognize (`--script` and
        /// `--record-home` still bind normally ahead of it), the same
        /// tolerance a real vendor CLI has for flags appended after its own.
        /// Order is preserved so the marker can be asserted on verbatim.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra: Vec<String>,
    },
    /// Reap dead test-harness state dirs under /tmp (see farhelm_teststate
    /// for the naming scheme and flock liveness protocol). Called by
    /// e2e/start-stack.sh at stack startup; the Rust integration tests
    /// reach the same sweep in-process. Best-effort and always exits 0 —
    /// a broken sweep must not block testing.
    SweepTestState,
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Cmd::FakeAgent {
            script,
            record_home,
            sync_output,
            transcript,
            then,
            exit_code,
            // The fake agent does not need to understand the injected
            // tail — it only has to survive parsing it. The record
            // scripts read the same strings straight from
            // `std::env::args()` for their `FAKE-AGENT ARGV:` marker,
            // so nothing is passed through here.
            extra: _,
        } => fake_agent::run(
            script,
            record_home,
            sync_output,
            fake_agent::ReplayOptions {
                transcript,
                then,
                exit_code,
            },
        ),
        Cmd::SweepTestState => {
            let outcome = farhelm_teststate::sweep(
                std::path::Path::new(farhelm_teststate::TMP_ROOT),
                &farhelm_teststate::SweepPolicy::default(),
            );
            farhelm_teststate::report(&outcome);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sweep verb parses — the exact invocation
    /// e2e/start-stack.sh performs, pinned here so a
    /// rename breaks a test instead of silently breaking the script's
    /// startup sweep.
    #[farhelm_testtrace::test]
    fn sweep_test_state_parses() {
        let cli = Cli::try_parse_from(["farhelm-fixtures", "sweep-test-state"]).unwrap();
        assert!(matches!(cli.command, Cmd::SweepTestState));
    }

    /// A supervisor that injects real-vendor hook flags past `--script`/
    /// `--record-home` must not turn the claude/codex-kind test fixtures
    /// into a clap usage error. This exercises a Claude-shaped tail
    /// (`--settings <json>`) immediately followed by a Codex-shaped one
    /// (`--dangerously-bypass-hook-trust -c a=b`) plus one more unknown
    /// flag pair (`--resume <id>`, purely a synthetic example of a tail
    /// the fixture has never heard of), and checks `extra` captures every
    /// one of those tokens untouched and in argv order — the exact
    /// property the `FAKE-AGENT ARGV:` marker relies on to let a test
    /// assert injection happened without the fixture parsing the injected
    /// flags itself.
    #[farhelm_testtrace::test]
    fn fake_agent_parses_with_injected_vendor_tail() {
        let cli = Cli::try_parse_from([
            "farhelm-fixtures",
            "fake-agent",
            "--script",
            "basic",
            "--settings",
            r#"{"x":1}"#,
            "--dangerously-bypass-hook-trust",
            "-c",
            "a=b",
            "--resume",
            "conv-1",
        ])
        .unwrap();
        let Cmd::FakeAgent { script, extra, .. } = cli.command else {
            panic!("expected Cmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::Basic));
        assert_eq!(
            extra,
            vec![
                "--settings",
                r#"{"x":1}"#,
                "--dangerously-bypass-hook-trust",
                "-c",
                "a=b",
                "--resume",
                "conv-1",
            ]
        );
    }

    /// Companion to the tail-injection test above: `trailing_var_arg`
    /// only starts collecting once clap can no longer match a known flag,
    /// so this pins that `--script` and `--record-home` still bind to
    /// their own fields — rather than being swallowed into `extra` — when
    /// an injected tail follows them on the same command line.
    #[farhelm_testtrace::test]
    fn fake_agent_named_flags_still_parse_before_the_tail() {
        let cli = Cli::try_parse_from([
            "farhelm-fixtures",
            "fake-agent",
            "--script",
            "claude-record",
            "--record-home",
            "/tmp/fake-agent-home",
            "--settings",
            "{}",
        ])
        .unwrap();
        let Cmd::FakeAgent {
            script,
            record_home,
            extra,
            ..
        } = cli.command
        else {
            panic!("expected Cmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::ClaudeRecord));
        assert_eq!(
            record_home,
            Some(std::path::PathBuf::from("/tmp/fake-agent-home"))
        );
        assert_eq!(extra, vec!["--settings", "{}"]);
    }

    /// Pins the `--sync-output` flag (`fake_agent::Script::FloodRegion`):
    /// that it parses to `true` when given, defaults to `false` when
    /// omitted, and — like `--record-home` — is accepted regardless of
    /// which script it accompanies, since every other script is
    /// documented to ignore it rather than reject it.
    #[farhelm_testtrace::test]
    fn fake_agent_sync_output_flag_parses_and_defaults_false() {
        let with_flag = Cli::try_parse_from([
            "farhelm-fixtures",
            "fake-agent",
            "--script",
            "flood-region",
            "--sync-output",
        ])
        .unwrap();
        let Cmd::FakeAgent {
            script,
            sync_output,
            ..
        } = with_flag.command
        else {
            panic!("expected Cmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::FloodRegion));
        assert!(sync_output);

        let without_flag =
            Cli::try_parse_from(["farhelm-fixtures", "fake-agent", "--script", "flood-region"])
                .unwrap();
        let Cmd::FakeAgent { sync_output, .. } = without_flag.command else {
            panic!("expected Cmd::FakeAgent");
        };
        assert!(!sync_output);

        // The "accepted regardless of script" half of the contract: a
        // script that ignores the flag must still parse with it present.
        let other_script = Cli::try_parse_from([
            "farhelm-fixtures",
            "fake-agent",
            "--script",
            "counter",
            "--sync-output",
        ])
        .unwrap();
        let Cmd::FakeAgent {
            script,
            sync_output,
            ..
        } = other_script.command
        else {
            panic!("expected Cmd::FakeAgent");
        };
        assert!(matches!(script, fake_agent::Script::Counter));
        assert!(sync_output);
    }
}
