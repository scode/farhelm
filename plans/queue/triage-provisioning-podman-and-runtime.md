# Provisioning, Podman caps, UI channel, and supervisor runtime fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Execute eleven settled backend and runtime `TRIAGE_OUTCOMES.md` entries per
`review_feedback_queue/.agents/execute-outcomes.md` with maintainer-approved grouped PRs (one linear 4-PR stack):

1. `uninstall-newline-stripping-defeats-state-inside-lib-guard.md` (`fix code`, highest)
2. `update-identity-unverified-misleading-plan-again-remedy.md` (`fix code`)
3. `installer-fallback-lock-world-openable.md` (`fix code`)
4. `trailing-podman-image-inspect-hits-output-cap.md` (`fix code`)
5. `trailing-podman-leftover-listing-hits-output-cap.md` (`fix code`)
6. `trailing-recipe-home-launch-skips-early-title-check.md` (`fix code`)
7. `ui-channel-displaced-notice-lost-to-reader-race.md` (`fix code`, clearly settled; ~100-line size gate)
8. `upload-cancel-mid-publication-reported-not-published.md` (`fix code`)
9. `trailing-tab-shell-hangup-delete-wiring-untested.md` (`other`, Nov 21 rule)
10. `trailing-replace-in-environment-history-check-vacuous.md` (`other`, Nov 21 rule)
11. `trailing-session-owned-setup-flag-untested.md` (`other`, Nov 21 rule)

Each PR graduates its covered entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-provisioning-podman-and-runtime/01-uninstall-update-and-installer-lock`) —
   `uninstall-newline-stripping-defeats-state-inside-lib-guard.md`,
   `update-identity-unverified-misleading-plan-again-remedy.md`, and `installer-fallback-lock-world-openable.md`:**
   - In `crates/farhelm-helm/src/provisioning/backend.rs`, replace `readlink -f -- "$1" 2>/dev/null | tr -d '\n'` with
     trailing-sentinel capture so embedded newlines are never stripped, and fail closed when a resolved path contains an
     embedded newline so Uninstall refuses the layout; add a regression test.
   - In `crates/farhelm-helm/src/provisioning/service.rs`, refuse Update planning when `row.host_identity` is `Some` and
     reported identity is `None` (`IdentityUnverified`) using the remedies already in `attach_refusal`, distinguish
     missing vs changed identity in `revalidate`, and add regression tests.
   - In `crates/farhelm-supervisor/src/installation/mod.rs`, create `FALLBACK_LOCK_FILE` with `.mode(0o600)` (leaving
     `~/Applications` creation standard) and add a unit test.
2. **PR 2 (`plan/triage-provisioning-podman-and-runtime/02-podman-projection-and-recipe-title`) —
   `trailing-podman-image-inspect-hits-output-cap.md`, `trailing-podman-leftover-listing-hits-output-cap.md`, and
   `trailing-recipe-home-launch-skips-early-title-check.md`:**
   - In `crates/farhelm-helm/src/environment/podman/build.rs`, define one shared `IMAGE_INSPECTION` `--format` template
     (mirroring `CONTAINER_INSPECTION` in `podman.rs`, noting `--format` returns a single JSON object `{...}` rather
     than an array) extracting `Id` and the needed `farhelm.*` labels, and use it in both `retire_images_unlocked` and
     `pull_base`, with unit test updates and coverage.
   - In `crates/farhelm-helm/src/environment/podman.rs`, switch `list_owned` to `--no-trunc --format {{.ID}}` while
     preserving the deliberate `try_join_all` all-or-error listing semantics, with unit tests.
   - In `crates/farhelm-helm/src/sessions.rs` (`CreateReq::prepare_sandbox_launch`), validate `self.title`
     unconditionally (printable check + 64 KiB cap with a neutral non-checkout error message) so invalid titles are
     refused before provisioning home-folder recipe environments as well as managed checkouts, with tests for both
     GUI/REST and agent-request routes.
3. **PR 3 (`plan/triage-provisioning-podman-and-runtime/03-ui-channel-drain-and-upload-cancel`) —
   `ui-channel-displaced-notice-lost-to-reader-race.md` and `upload-cancel-mid-publication-reported-not-published.md`:**
   - In `crates/farhelm-helm/src/ui_channel.rs` (`serve_socket`), ensure queued outgoing frames (especially
     `ServerMessage::Displaced`) finish draining after `core` returns without an incoming client frame aborting the
     drain, preserving `SILENCE_CLOSE`, reader-first, and write-failure teardowns, with a regression test sending a
     concurrent incoming frame during displacement (respecting its ~100-line clearly-settled size gate).
   - In `crates/farhelm-supervisor/src/service/uploads.rs` (`commit_upload`), return `ErrorKind::Internal` for
     `DiskStage::Cancelled` (matching `DiskStage::Failed` and `SPEC_impl.md:5050`) so unknown publication outcomes are
     never mapped to `NotPublished`, with a focused unit test.
4. **PR 4 (`plan/triage-provisioning-podman-and-runtime/04-runtime-test-gap-checks`) —
   `trailing-tab-shell-hangup-delete-wiring-untested.md`, `trailing-replace-in-environment-history-check-vacuous.md`,
   and `trailing-session-owned-setup-flag-untested.md`:**
   - Apply the Nov 21 rule to each of the three test-only gaps independently: fix the test if the fix is a few lines
     (about 20 lines at most) and guards user-facing behavior; otherwise make no test change. Graduate all three entries
     to `TRIAGE_OUTCOMES_ARCHIVE.md` recording which way each went and remove their queue files and index lines.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **Nov 21 rule on PR 4 test gaps:** Fix only if about 20 lines or fewer and guarding user-facing behavior; otherwise
  remove with no code change and record which way it went.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation"),
  `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PRs above),
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1:** Reuse existing `attach_refusal` `IdentityUnverified` wording in `provisioning/service.rs`; keep the
   `readlink -f` sentinel check inside the existing uninstall probe script in `provisioning/backend.rs`.
2. **PR 2:** Share one `IMAGE_INSPECTION` template constant in `podman/build.rs` (remembering `--format` emits a JSON
   object, not a 1-element array). For the 64 KiB session-title limit, share a tiny `MAX_SESSION_TITLE_BYTES` constant
   in `crates/farhelm-proto/src/text.rs` (keeping `check_checkout_preview_title`'s existing message if pinned by tests)
   instead of duplicating a magic `64 * 1024`.
3. **PR 3:** Flatten the two nested `tokio::select!` blocks in `serve_socket` (`ui_channel.rs`) into one
   `tokio::select!` over `core`, `&mut writer`, and `reader` — awaiting `writer` to drain only in the `core` arm — with
   no new cancellation token or channel.
4. **PR 4:** Evaluate each of the 3 test gaps honestly against the ~20-line Nov 21 bar (`sessions_tests.rs` assertions
   for `needs_checkout_root` and `RepositoryOnly` are ~10-20 lines; `sweep_roots` wiring may be close to or above the
   bar — drop without forcing it if it exceeds ~20 lines).

## Scope reassessment

Before implementing a substantial departure from the outline (or if repeated corrective reviews hit the same component),
run a fresh-context review at your own model and effort with this charter verbatim:

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

Supply the request, decisions, outline, current diff, and proposed departure. A close reuse-versus-duplication call
within agreed scope is decided after reassessment and logged as `DECISION`. If a same-model sub-agent discovers an
unapproved new mechanism mid-unit, leave that part unimplemented and report it rather than inventing it.

## How to run

### Checkout check

This goal runs inside the checkout the executing session runs in, which must be a checkout of GitHub repository
`scode/farhelm` (the nearest `.git` or `.jj` checkout containing the working directory). Confirm that first. If it is
not, stop before the resume protocol, write nothing (not even the log), and report the mismatch. Every project path in
this file is relative to that checkout's root.

### Resume protocol

Right after confirming the checkout, invoke the `agent-resumeable` skill with the working log at
`farhelm-plan-triage-provisioning-podman-and-runtime-log.md` in the parent directory of the executing checkout. If the
log already exists, read it and resume where the previous session left off (cross-checking against VCS state and open
PRs) rather than starting over. If it does not exist, this is a fresh start.

### Execution mode

No-workhorse mode (explicit). Do not send any unit of your own decomposition to a weaker model or lower effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active, stop it). Same-model sub-agents at your own effort are allowed at your judgment when
they speed up independent work without losing quality (using separate worktrees or jj workspaces if concurrent); you
still own the `jjstack` stack and every PR. The sub-agents required by `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any delegation restriction. Scope
reassessment runs at your own model and effort; the per-PR review gate keeps its demanded model and effort below.

### PR discipline

Build a linear stack of reviewable draft PRs using the `jjstack` skill, based on `main@origin` (or this plan's own open
PRs on resume, never another plan's PRs), per `plans/.agents/executing.md`, "Executing one plan". Use bookmarks
`plan/triage-provisioning-podman-and-runtime/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
merge. Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on
top. Graduate each covered outcome to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove its feedback file and
`review_feedback_queue/INDEX.md` entry in the same PR.

### Review gate

Before finishing any PR in this stack, run a fresh-context review of its changes using a **single fresh-context agent on
`gpt-6.2-sol` at `high` effort** (launched natively if available or through `scode-harness-shellout`, never hardcoding a
launch command here). Pass the executing checkout root, commit range or bookmark, and a private findings file in your
scratch directory, plus this full charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the supplied file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

When the PR changes tests or test fixtures, also supply `.agents/test-authoring.md` verbatim per `AGENTS.md`, "Tests and
validation". Address all valid findings before moving on.

### Resource watchdog

Right after the resume protocol and before launching sub-agents or builds, start the `scode-build-goal` skill's
`scripts/watchdog` (resolved via `realpath` next to that skill's `SKILL.md`), with status file
`farhelm-plan-triage-provisioning-podman-and-runtime-watchdog.json` beside the working log, watching the executing
checkout root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other
filesystems, at a 60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog
section for harness wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, clearly-settled size gate exceeded, or unresolved design conflict),
push all consistent work and block per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things
the maintainer should know and possible follow-ups go into the delivery report.

### Done criterion

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.
