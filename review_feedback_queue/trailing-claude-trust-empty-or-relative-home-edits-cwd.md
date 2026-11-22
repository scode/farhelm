# Empty or relative HOME in Claude trust resolution edits .claude.json inside the working directory

Reviewed commit: b3a14cd25cc48869e18bc6be867d47822251a15c

## TLDR

When Claude workspace trust is enabled and the session shell's HOME variable is empty (or relative) with no custom
Claude config directory set, Farhelm looks for and edits `.claude.json` (or `.claude/.config.json`) inside the session's
working directory instead of refusing an unusable home path. Because Claude trust now runs in arbitrary user-chosen
directories, this can modify a file inside a repository or follow a symlink placed there.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:39`.

Bucket note: placed in `other` per the established-route rule because no shipped Farhelm control produces empty/relative
`HOME`; would move to `highest` if a supported host or container shell runs with empty/relative `HOME` inside an
untrusted directory.

1. In `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:37-46`, `Environment::current()` filters empty values
   for `CLAUDE_CONFIG_DIR` (line 41) and `CLAUDE_CODE_CUSTOM_OAUTH_URL` (line 44), but reads `HOME` with
   `std::env::var_os("HOME").context("HOME is unset")?.into()` (line 39) without rejecting empty or non-absolute paths.
2. When `HOME=""` (and `CLAUDE_CONFIG_DIR` is unset/empty), `Environment::config_path()` (`trust.rs:51-75`) computes
   `self.home.join(".claude.json")` -> relative `.claude.json` and `self.home.join(".claude").join(".config.json")` ->
   relative `.claude/.config.json`.
3. In `exec_launch_spec_with_seam` (`crates/farhelm-supervisor/src/launch.rs:1211-1235`), the shim has already chdir'd
   into `spec.working_directory` before calling `claude_trust_covers_too_much` and
   `apply_claude_workspace_trust(Path::new("."), ...)`. With `HOME=""`, `std::fs::canonicalize("")` fails so
   `claude_trust_covers_too_much` returns `false`, and `trust::resolve` canonicalizes `./.claude.json` inside `cwd` and
   edits it in place.
4. Fix direction: In `trust::Environment::current()` (`trust.rs:39`), filter out empty `HOME` (`!v.is_empty()`) and
   require `Path::new(&home).is_absolute()`, returning an error when empty or relative so the best-effort trust edit
   fails closed without touching relative paths in `cwd`.

Reviewer metadata (g15-trust-claude-specs-helm-sup/l5): suggested bucket: other (could be in highest: only if a shipped
control or environment produces empty/relative HOME when launching inside an untrusted directory containing a
.claude.json file or symlink, in which case Farhelm would mutate an arbitrary file in cwd or its symlink target and also
bypass the HOME guard; under ordinary host environments HOME is non-empty and absolute, so established bucket is other);
confidence: confirmed; possible cover: overlaps g16-trust-sessions-and-claude-guard l8.

Additional source record at `6f9918a88100c8d8a69ae471fe12cfde4c7bf76d`, `crates/farhelm-supervisor/src/launch.rs:1226`
(g16-trust-sessions-and-claude-guard/l8):

In `crates/farhelm-supervisor/src/launch.rs:1226`, `exec_launch_spec_with_seam` reads
`let home = std::env::var_os("HOME").map(std::path::PathBuf::from);` without filtering empty values, and
`claude_trust_covers_too_much` (`launch.rs:2026-2028`) ignores `std::fs::canonicalize("")` errors and returns `false`.
Immediately afterward, `apply_claude_workspace_trust` (`launch.rs:2039`) calls `trust::Environment::current()`
(`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:37-46`), which filters empty strings for `CLAUDE_CONFIG_DIR`
(line 41) and `CLAUDE_CODE_CUSTOM_OAUTH_URL` (line 44) but does NOT filter empty `HOME` (line 39:
`std::env::var_os("HOME").context("HOME is unset")?.into()`). Consequently, when `HOME=""`:

- If `CLAUDE_CONFIG_DIR` is set, `trust::resolve` succeeds and edits `$CLAUDE_CONFIG_DIR/.claude.json` even if `cwd` is
  the user's real home directory, because `claude_trust_covers_too_much` failed to canonicalize `""`.
- If `CLAUDE_CONFIG_DIR` is unset, `Environment::config_path()` (`trust.rs:51-75`) computes
  `PathBuf::from("").join(".claude.json")` = relative `.claude.json` (and `.claude/.config.json`) inside `cwd`! If the
  session's working directory is an untrusted repository that contains a `.claude.json` file, Farhelm modifies that
  repository file instead of refusing empty `HOME`. Fix direction: In `trust::Environment::current()`
  (`agent_kind/claude/trust.rs:39`), filter out empty (and non-absolute) `HOME` values
  (`std::env::var_os("HOME").filter(|v| !v.is_empty())` and `ensure!(home.is_absolute(), ...)`), and in
  `launch.rs:1226-1234` fail closed if `home` is absent/empty or cannot be canonicalized rather than proceeding to edit
  Claude's settings file.

Reviewer metadata (g16-trust-sessions-and-claude-guard/l8): suggested bucket: other (would move to highest if a
supported host or container login shell runs with HOME="" alongside CLAUDE_CONFIG_DIR or an untrusted directory
containing .claude.json — currently no shipped Farhelm control produces HOME="", so bucketed as other per the
established-route rule); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g15-trust-claude-specs-helm-sup/l5`,
`g16-trust-sessions-and-claude-guard/l8`.
