# Plan: Regression test for Claude drift trust copy-back protection

## Goal

Add a focused regression test in `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs` proving that when
a mirrored `.claude.json` with stripped ancestor trust entries is atomically replaced (temp file + rename, changing its
inode) while the real `~/.claude.json` is unchanged, `PrivateConfig::finish(true)` never copies the stripped file back
into the real Claude configuration.

Acceptance criteria:

- One standalone regression test (`#[farhelm_testtrace::test]`) in
  `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs` verifies that:
  1. Real `home/.claude.json` starts with ancestor directory `hasTrustDialogAccepted: true`.
  2. `PrivateConfig::claude_trust_mirrors` strips ancestor trust in `private-home/.claude.json`.
  3. An atomic temp-file + `fs::rename` rewrite of `private-home/.claude.json` (confirming the inode changed) simulates
     a live Claude config save while real `home/.claude.json` remains untouched.
  4. `config.finish(true)` returns empty notes and leaves real `home/.claude.json` byte-for-byte unchanged with ancestor
     trust still `true`.
- No production or fixture code changes are made unless the test unexpectedly reveals a real bug (which would trigger
  scope reassessment).
- The last code PR removes the
  `Verify and guard Claude drift trust copy-back against clearing real parent-directory trust.` entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** Check whether `copy_back` in `private_config.rs` can copy back the stripped Claude
  trust file into the real Claude configuration when Claude rewrites it during a drift run, and if it can, guard against
  it and add a regression test.
- **Verification completed at planning time:** Verified in `private_config.rs` and
  `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs` that all six Claude trust config paths stay in
  `self.copied` (never `self.links`), aliases are copied rather than linked, `copy_back` iterates only `&self.links`,
  and `replace_real` independently rejects any `(dev, ino)` in the `protected` set of `mirror.copied`. Existing test
  `claude_trust_mirror_clears_applicable_flags_and_checks_only_new_real_workspace_trust` mutated `real` right before
  `finish(true)`, leaving the unchanged-real atomic-rename scenario unpinned.
- **Settled maintainer decisions:**
  - Queue a small plan to add the focused atomic-rename regression test in `private_config.rs` and remove the TODO
    entry.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `AGENTS.md` ("Conventional Commits", "Tests and validation"),
  `.agents/test-authoring.md`, and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Keep the new test standalone (~25–35 lines) using the same inline `farhelm_teststate::tempdir()`,
    `PrivateConfig::claude_trust_mirrors`, and `claude_project_is_trusted` setup as the existing Claude trust tests in
    `private_config.rs`; do not extract a new test helper or refactor neighboring tests.
  - Do not add redundant `CLAUDE_CONFIG_DIR` or symlinked-config test variants (already covered by
    `claude_trust_copies_an_explicit_symlinked_config_and_keeps_its_lock_private`).
  - Single PR (`plan/claude-drift-trust-copy-back/01-atomic-rename-regression-test`).

## Implementation outline

Single tiny test-only PR:

1. In `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs`, add one standalone
   `#[farhelm_testtrace::test]` matching the existing inline setup style in `mod tests`:
   - Create `tempdir`, real `home/.claude.json` with a `parent` directory having `hasTrustDialogAccepted: true`, and a
     child workspace `parent/project`.
   - Build `PrivateConfig::claude_trust_mirrors` for `parent/project`, assert `private-home/.claude.json` stripped
     `parent` trust, record the mirrored file's initial inode, write updated JSON to a sibling temp file, and
     `fs::rename` it over `private-home/.claude.json` (asserting the inode changed).
   - Leave real `home/.claude.json` untouched, call `config.finish(true)`, and assert it returns `Ok(notes)` with
     `notes.is_empty()` and that real `home/.claude.json` is byte-for-byte identical to `before` (with
     `claude_project_is_trusted` still `true` for `parent`).
2. Remove the covered entry from `TODO.md`. (Test-only `test:` PR; no changelog fragment needed.)

## Scope reassessment

Before implementing a substantial departure from the outline — or after repeated corrective reviews of the same
component — run a fresh-context review at the executing agent's own model and effort with this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

Supply the request, settled decisions, outline, current diff, and proposed departure (what changed, why it is necessary,
and which simpler alternative was ruled out). A routine fork within the agreed scope is decided after reassessment and
logged as `DECISION`. Any material scope expansion, weakened guarantee, or omitted required behavior follows the
unattended fallback below.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in, which must be the nearest Git worktree or
   jj workspace containing the working directory (a `.git` file counts as well as `.git`). Confirm a remote points to
   `scode/farhelm` (compared as host, owner, and name). If not, stop before the resume protocol, write nothing, and
   report the mismatch. Every project path in this plan is relative to that checkout root.
2. **Resume protocol:** Resolve the plan's working log directory with
   `cargo xtask records path plan/claude-drift-trust-copy-back` (`plans/.agents/outside-files.md`, "Files outside the
   repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
   exists, read all of it and resume from where the previous session left off, cross-checking against GitHub and VCS
   state (`plans/.agents/executing.md`, "Executing one plan", step 7); otherwise start fresh.

## Execution mode and resource watchdog

- **No-workhorse mode:** This is an explicit no-workhorse choice. Never send any unit of your own decomposition to a
  weaker model or lower effort than your own, and never invoke, load, or follow `scode-galaxy-brain` or
  `scode-model-routing` at any point (if `scode-galaxy-brain` is already active, stop it). Same-model, same-effort
  sub-agents are allowed at your judgment when likely to finish sooner without costing quality (using separate worktrees
  or jj workspaces if concurrent writers would conflict, while you retain ownership of `jjstack` and every PR). The
  sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires", the per-PR review gate below, and any scope
  reassessment review are exempt from any no-delegation rule. Launch sub-agents natively where supported, or through
  `$scode-harness-shellout` when the demanded model is unavailable natively.
- **Resource watchdog:** Right after the resume protocol and before any delegation, start the `scode-build-goal` skill's
  `scripts/watchdog` (resolved with `realpath` at run time) using
  `farhelm-plan-claude-drift-trust-copy-back-watchdog.json` in the parent directory of the executing checkout (never
  inside the `plan/claude-drift-trust-copy-back` record directory). Watch the executing checkout root, `$HOME`, `/tmp`,
  the parent directory holding the status file, your scratch directory, and any worktree or build directory on another
  filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What the goal file must
  contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at boundaries, acting
  on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/claude-drift-trust-copy-back/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/claude-drift-trust-copy-back/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Tests and validation"),
  `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the docstring/comment rules with
  `scode-voice`. Run the targeted drift `private_config` test selection
  (`cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` through
  `python3 scripts/record-test-run.py`), `cargo fmt`, `cargo clippy`, `dprint check`, and
  `python -B scripts/check-test-sleeps.py`.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. Because this PR adds a Rust test, also supply `.agents/test-authoring.md`
  verbatim to the reviewer. Address findings before moving on.

## Decision logging and unattended fallback

- Log consequential implementation choices in `log.md` as they happen with a scannable `DECISION` label and the
  alternatives considered.
- Never ask the maintainer anything in conversation — no direct questions, no harness question prompts, no optional
  next-step offers, and no waiting for confirmation (`plans/AGENTS.md`, "Unattended means no questions"). Decide routine
  implementation forks within the agreed scope, log them as `DECISION`, and carry on. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold. If a material scope
  expansion, weakened guarantee, omitted required behavior, or decision only the maintainer can make comes up, block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put things the maintainer should know and
  possible follow-ups in the report.

## Done criterion

Every PR in the stack exists as a draft PR on GitHub, satisfies the acceptance criteria (including removing the
`TODO.md` entry in the last code PR), passes its chosen validation and its Opus 5.5 high review gate, and satisfies the
latest `## Decisions` entry, if any. Delivering the report is the executor's closing step (`plans/.agents/executing.md`,
"Executing one plan", step 11), not part of this goal.

## Release note

No release note needed.
