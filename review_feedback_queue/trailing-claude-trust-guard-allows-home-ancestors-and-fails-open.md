# Claude workspace trust skips home and root but still trusts ancestors of home and fails open

Reviewed commit: 6f9918a88100c8d8a69ae471fe12cfde4c7bf76d

## TLDR

With Claude Code's automatic workspace-trust setting enabled, starting a Claude session in a parent of your home
directory (such as /home on Linux or /Users on macOS) still writes that directory as trusted in Claude's settings file
with no removal record. Because Claude treats any trusted directory as covering everything beneath it, this permanently
trusts your entire home directory and every repository inside it—both outside Farhelm and after turning the setting
off—defeating the protection added for the home directory and filesystem root. In addition, if HOME cannot be resolved
(for example a broken symlink or empty value while CLAUDE_CONFIG_DIR is set), the guard fails open instead of skipping
the edit.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-supervisor/src/launch.rs:2021-2029`.

In `crates/farhelm-supervisor/src/launch.rs:2021-2029`, `claude_trust_covers_too_much(cwd, home)` only checks
`folder.parent().is_none() || home.and_then(|home| std::fs::canonicalize(home).ok()).is_some_and(|home| home == folder)`.

1. Ancestors of HOME: For any proper ancestor of `$HOME` (e.g., `/home` when `HOME=/home/alice`, `/Users` when
   `HOME=/Users/alice`, `/var/home` on Fedora Silverblue, or `/home/org` when `HOME=/home/org/alice`), `folder.parent()`
   is `Some("/")` and `home == folder` is `false`, so `exec_launch_spec_with_seam` (`launch.rs:1226-1234`) proceeds to
   `apply_claude_workspace_trust(Path::new("."), |_| Ok(()))` and writes `hasTrustDialogAccepted: true` for that
   ancestor in `~/.claude.json` (`agent_kind/claude/trust.rs:241-249`) with no cleanup record. Under the exact Claude
   prefix-trust behavior cited in `SPEC.md:1782-1784` and `launch.rs:2016-2018` ("Claude treats a trusted directory as
   covering every directory inside it"), trusting an ancestor of `$HOME` trusts everything under `$HOME` plus sibling
   user directories—strictly broader than trusting `$HOME` itself.
2. Canonicalization failure behavior: If `std::fs::canonicalize(cwd)` fails, returning `false` is safe because
   `trust::resolve` (`trust.rs:100`) also calls `fs::canonicalize(cwd)?` and aborts before editing. However, if
   `std::fs::canonicalize(home)` fails (broken symlink, missing directory, unreadable parent, relative path, or empty
   string) while `CLAUDE_CONFIG_DIR` points to an existing config directory, `claude_trust_covers_too_much` returns
   `false` (fails open) and `trust::Environment::current()` / `trust::resolve` (`trust.rs:38-75`) succeeds using
   `config_dir` without touching `home`, allowing the edit to proceed unguarded. (When `HOME` is completely unset,
   `Environment::current()` fails at `trust.rs:39`.) Note: `SPEC.md:1782-1784` names only the home directory and
   filesystem root as skipped, so this is both a code guard gap and a spec wording gap for ancestors of home. Fix
   direction: Check `home.starts_with(&folder)` (path-component prefix comparison, so `/home/al` does not match
   `/home/alice`, while `/`, `/home`, `/Users`, and `$HOME` all match) alongside `folder.parent().is_none()`; fail
   closed in the launch-time guard if `home` is absent or cannot be canonicalized; update the terminal notice and
   `SPEC.md` wording; and add unit tests in `launch.rs:2490` covering `home.path().parent()` and unresolvable `home`.

Reviewer metadata (g16-trust-sessions-and-claude-guard/l1): suggested bucket: highest (could be in highest: security —
permanently writes Claude workspace trust for a parent of $HOME with no removal record, silently trusting every
repository and directory inside $HOME and other home directories under Claude's prefix-trust model, including outside
Farhelm and after turning the switch off); confidence: confirmed; possible cover: none.

Additional source record at `b3a14cd25cc48869e18bc6be867d47822251a15c`, `SPEC.md:1782`
(g15-trust-claude-specs-helm-sup/f2):

1. Commit `b3a14cd2` extended Claude's trust edit to arbitrary user-chosen directories via
   `LaunchSpec.claude_workspace_trust` (`crates/farhelm-supervisor/src/launch.rs:1217-1235`) with no removal record.
2. At pinned tip `66ad3aaa`, both `SPEC.md:1782-1784` ('Farhelm does not make the edit when a session starts in the home
   directory or the filesystem root: Claude treats a trusted directory as covering every directory inside it, so either
   would trust far more than the session's directory, and Claude asks there as usual') and
   `claude_trust_covers_too_much` (`crates/farhelm-supervisor/src/launch.rs:2021-2029`) check only
   `folder.parent().is_none() || home == folder`.
3. Any proper ancestor of `$HOME` (for example `/home` on Linux or `/Users` on macOS) satisfies neither condition, so
   `apply_claude_workspace_trust` writes `projects["/home"].hasTrustDialogAccepted = true` (or `"/Users"`) into
   `~/.claude.json` with no archive cleanup record. Because Claude treats a trusted directory as covering all
   subdirectories, trusting an ancestor of `$HOME` trusts `$HOME` and every repository inside it (strictly broader than
   trusting `$HOME` itself).
4. Note also that `SPEC_impl.md:3510-3515` (updated in `b3a14cd2`) does not mention the over-broad-directory guard at
   all.
5. Fix direction: Update `claude_trust_covers_too_much` (`launch.rs:2021-2029`) to reject any directory that is an
   ancestor of (or equal to) `$HOME` (`home.starts_with(&folder)` covers `/`, `/home`, `/Users`, and `$HOME` itself,
   while keeping `folder.parent().is_none()` when `HOME` is absent/unresolvable), and update `SPEC.md:1782-1784` (and
   `SPEC_impl.md:3510-3515`) to specify excluding the home directory and its ancestors (including the filesystem root).

Reviewer metadata (g15-trust-claude-specs-helm-sup/f2): suggested bucket: highest (could be in highest: yes —
security/trust-boundary defect with an established shipped route: starting a Claude session in /home or /Users with
Claude trust enabled permanently marks that ancestor directory trusted in ~/.claude.json with no removal record,
trusting $HOME and every directory beneath it both inside and outside Farhelm even after the setting is turned off);
confidence: confirmed; possible cover: overlaps g16-trust-sessions-and-claude-guard l1 (code guard); adds the
SPEC.md:1782-1784 / SPEC_impl.md:3510-3515 contract gap.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g16-trust-sessions-and-claude-guard/l1`, `g15-trust-claude-specs-helm-sup/f2`.
