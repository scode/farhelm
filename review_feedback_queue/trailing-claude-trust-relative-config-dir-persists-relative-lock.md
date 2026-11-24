# Relative CLAUDE_CONFIG_DIR is accepted by Environment::from_vars, allowing repo-relative symlink edits and persisting a relative lock_path

Reviewed commit: a4cdd1bbde2f2a95424bab65b9b710deece834cd

## TLDR

When a user's login shell sets a relative CLAUDE_CONFIG_DIR, Farhelm's automatic Claude folder-trust setup resolves
Claude's settings inside the session or cloned repository directory—following any symlink there to rewrite an arbitrary
user-owned JSON file without holding its real lock—and for managed checkouts saves a relative lock path that later fails
or locks the wrong directory in the supervisor's working directory during checkout deletion.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:47-61`.

- In `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:47-61`, commit `a4cdd1bb` extracted
  `Environment::from_vars` specifically to guard against empty or relative `HOME` (noting in its doc comment at lines
  47-48: "An empty or relative home could also select a config in the session's working directory"), enforcing
  `home.is_absolute()`. However, `config_dir` on line 58 (`config_dir.filter(|v| !v.is_empty()).map(Into::into)`)
  accepts relative paths without checking `is_absolute()`.
- On BOTH ordinary launches (`crates/farhelm-supervisor/src/launch.rs:1212-1227`, after `enter_session_directory`) and
  fresh managed checkouts (`crates/farhelm-supervisor/src/service/core/host_launch.rs:529`, which spawns the terminal
  child with `cwd: PathBuf::from(&cwd)`, followed by `launch.rs:1987`), the shim's current working directory when
  `trust::resolve` runs is the session/cloned repository directory.
- In `trust::resolve` (`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:107-125`),
  `selected = environment.config_path()` returns a relative path when `config_dir` is relative (e.g.
  `.claude/.claude.json`). `config_path` is canonicalized via `fs::canonicalize(&selected)`, following symlinks inside
  the session/cloned repo to any target regular file, while `lock_path` is built directly from the uncanonicalized
  `selected` (`selected.into_os_string()` + `.lock`), producing a relative `TrustRecord.lock_path`.
- Consequence 1 (untrusted-repo symlink rewrite / lockless edit before Claude starts): If a user with a relative
  `CLAUDE_CONFIG_DIR` (e.g. `.claude`) opens or clones an untrusted repository containing a symlink
  `.claude/.claude.json` pointing to `~/.claude.json` or another user-writable JSON object file, `trust::edit`
  (`trust.rs:226-307`) acquires only `<repo>/.claude/.claude.json.lock` and atomically rewrites the symlink target as
  pretty-printed JSON with `projects[<cwd>].hasTrustDialogAccepted = true`. Against `~/.claude.json` this bypasses
  Claude's real `~/.claude.json.lock` and races concurrent locked writes (sign-in/OAuth/settings); against another user
  JSON file it mutates/re-serializes an unrelated file outside the repo before the user ever answers a trust prompt.
- Consequence 2 (managed checkout retirement in supervisor cwd): `run_checkout_preparation` (`launch.rs:1987-1997`)
  persists `TrustRecord` with the relative `lock_path` into `PreparationRecord.claude_trust` on disk. Later,
  `remove_preparation_trust_sync` (`launch.rs:1664-1689`, called from `teardown.rs:981` and startup recovery
  `service/core.rs:6416`) runs in the supervisor daemon process, whose working directory is unrelated to the checkout
  (e.g. `$HOME` under systemd, where `~/.claude` usually exists). `ConfigLock::acquire` then either fails with `ENOENT`
  (leaving the trust entry in place and retaining the preparation state file to fail on every supervisor startup) or
  creates/removes a lock directory relative to the supervisor's cwd (e.g. `$HOME/.claude/.claude.json.lock`) while
  editing canonical `config_path` without holding the target's lock.
- Note: `config_dir` was already accepted without `is_absolute()` prior to `a4cdd1bb`, but `a4cdd1bb` introduced
  `Environment::from_vars` specifically to close the relative-path config selection hazard for `HOME` while leaving
  `config_dir` open on the next line.
- How to verify: call `Environment::from_vars(Some("/home/alice".into()), Some(".claude".into()), None)` followed by
  `resolve(&env, &project)` with a relative `.claude/.claude.json` (or symlink to a target JSON file) in cwd and observe
  `record.lock_path.is_absolute() == false` while `record.config_path` points at the canonical symlink target.
- Fix: reject non-absolute `CLAUDE_CONFIG_DIR` in `Environment::from_vars`
  (`if let Some(dir) = &config_dir { ensure!(dir.is_absolute(), "CLAUDE_CONFIG_DIR is relative"); }`), matching `HOME`'s
  check so `apply_claude_workspace_trust` skips the edit with a clear terminal notice and never resolves inside the
  session directory or persists a relative `lock_path`.

Reviewer metadata (g14_claude_trust_home_and_whats_new/f1): suggested bucket: highest (reassessed from `other` in Turn 2
after confirming `host_launch.rs:529` sets the managed-checkout shim cwd to the cloned repo too: the route from a
relative `CLAUDE_CONFIG_DIR` + untrusted repo symlink to lockless pre-prompt rewrite of `~/.claude.json` or an arbitrary
user-owned JSON object file outside the repo, plus unlocked supervisor-side cleanup creating/removing lock dirs relative
to supervisor cwd, is fully established in shipped code. Bucket boundary uncertainty: reaching the security/data-loss
consequence requires the user's own login shell to export a relative `CLAUDE_CONFIG_DIR`, in which case Claude Code
itself would also read `CLAUDE_CONFIG_DIR` from cwd, though Claude would not edit before its trust prompt or persist a
relative lock path into Farhelm's supervisor teardown; if triage treats a user-exported relative `CLAUDE_CONFIG_DIR` as
outside the security boundary, this drops to `other`).; confidence: confirmed (supported by source analysis of
`trust.rs:47-125,226-307`, `host_launch.rs:529`, `launch.rs:1212-1227,1664-1689,1987-1997`, `teardown.rs:975-993`,
`service/core.rs:6405-6425`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g14_claude_trust_home_and_whats_new/f1`.
