# Spec and code-comment accuracy updates

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Execute nine settled docs/spec/comment-only `TRIAGE_OUTCOMES.md` entries (with zero runtime behavior changes) per
`review_feedback_queue/.agents/execute-outcomes.md` in one maintainer-approved grouped PR:

1. `trailing-legacy-claude-settings-v036-hosts-never-cleaned.md` (`fix spec`, highest)
2. `trailing-legacy-codex-config-v036-hosts-never-cleaned.md` (`fix spec`, highest)
3. `trailing-spec-file-click-release-wording.md` (`fix spec`)
4. `trailing-spec-impl-hook-answer-preview-wait.md` (`fix spec`)
5. `trailing-spec-impl-custom-agent-id-letter.md` (`fix spec`)
6. `trailing-stale-references-to-removed-repository-search.md` (`fix spec+code` — docs/comments only)
7. `shortcuts-chord-during-composition.md` (`fix spec`)
8. `trailing-grid-wide-glyph-absorbs-foreign-spacer.md` (`fix spec`)
9. `trailing-symlinked-claude-json-permanent-warning.md` (`fix spec`)

The PR graduates all nine entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-spec-and-comment-updates/01-spec-and-comment-updates`) — all 9 entries:**
   - Update `SPEC_impl.md` ("Named credential catalog") to state clearly that **Tensorlake is still experimental** and
     that legacy v0.36.0 Tensorlake hosts that never reconnected under v0.37–v0.43 (carrying no delivery record) are
     accepted as not automatically migrated for both Claude settings and Codex config.
   - Reword `SPEC.md` file-click release text (around line 3753) to match shipped behavior and `SPEC_impl.md` (pressed
     while that hover's disclosure is painted and fully visible, and released before the disclosure is replaced, with
     link text unchanged).
   - Narrow `SPEC_impl.md` (around line 1005) so it accurately distinguishes the YOLO answer (carries permission through
     a host-setting write or checkout preview) from the hook-setup answer (carries permission through its setting write
     and refuses safely if a checkout preview is still loading).
   - Fix `SPEC_impl.md` (around line 7613) from "start with a letter" to "start with a lowercase letter or digit" for
     registered custom agent IDs.
   - Remove all 6 stale references to removed host-side repository discovery/search across `SPEC.md`,
     `crates/farhelm-supervisor/src/service/connection.rs`, `crates/farhelm-supervisor/src/working_copies.rs`,
     `crates/farhelm-proto/src/lib.rs`, and `crates/farhelm-helm/src/ui_channel_tests.rs`.
   - Document the Linux XKB compose-cancellation limitation for `Ctrl+Shift+/` in `SPEC_impl.md` (no vendored GPUI
     patch).
   - Add mid-wide-character DCH/ICH edits (deleting/inserting starting on a wide character's spacer column) to
     `SPEC.md`'s 2026-11-21 terminal fidelity exclusions.
   - Update `SPEC_impl.md` (around line 6836) to accept the generic persistent warning and retry on symlinked or
     non-regular `~/.claude.json` alongside the existing 1 MiB trade-off.
   - Follow `AGENTS.md`, "Tests and validation", for docs/comment-only changes: run `dprint check` on changed Markdown,
     `python3 scripts/check-agents-md.py` if any heading changed, and `cargo fmt --all -- --check` (and avoid running
     Rust/JS/GUI runtime suites for comment-and-Markdown-only edits).

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and single grouped PR:** Approved by the maintainer on 2026-11-25.
- **v0.36.0 Tensorlake Claude & Codex cleanup:** _"tensorlake is STILL experimental to be clear. Yes. accept this, fix
  spec."_
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Tests and validation" — do not run runtime tests for docs/comments-only
  changes), `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PR above), and
  `.agents/validation.md`.
- Do not modify frozen historical entries in `TRIAGE_OUTCOMES_ARCHIVE.md` (append-only).

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

- Pure prose edits in `SPEC.md`, `SPEC_impl.md`, and the 4 Rust comment/docstring files, plus the queue/ledger
  graduation for all 9 entries. No production code or runtime tests change.

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

Supply the request, decisions, outline, current diff, and proposed departure.

## How to run

### Checkout check

This goal runs inside the checkout the executing session runs in, which must be a checkout of GitHub repository
`scode/farhelm` (the nearest `.git` or `.jj` checkout containing the working directory). Confirm that first. If it is
not, stop before the resume protocol, write nothing (not even the log), and report the mismatch. Every project path in
this file is relative to that checkout's root.

### Resume protocol

Right after confirming the checkout, invoke the `agent-resumeable` skill with the working log at
`farhelm-plan-triage-spec-and-comment-updates-log.md` in the parent directory of the executing checkout. If the log
already exists, read it and resume where the previous session left off (cross-checking against VCS state and open PRs)
rather than starting over. If it does not exist, this is a fresh start.

### Execution mode

No-workhorse mode (explicit). Do not send any unit of your own decomposition to a weaker model or lower effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active, stop it). Same-model sub-agents at your own effort are allowed at your judgment when
they speed up independent work without losing quality; you still own the `jjstack` stack and PR. The sub-agents required
by `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked questions and reports), are
exempt from any delegation restriction. Scope reassessment runs at your own model and effort; the per-PR review gate
keeps its demanded model and effort below.

### PR discipline

Build one clean draft PR using the `jjstack` skill, based on `main@origin` (or this plan's own open PR on resume, never
another plan's PRs), per `plans/.agents/executing.md`, "Executing one plan". Use bookmark
`plan/triage-spec-and-comment-updates/01-spec-and-comment-updates`. Keep the PR as a draft — never mark it ready and
never merge. Never edit files under `plans/`. Graduate all 9 covered outcomes to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove
their feedback files and `review_feedback_queue/INDEX.md` entries in the same PR.

### Review gate

Before finishing the PR, run a fresh-context review of its changes using a **single fresh-context agent on `gpt-6.2-sol`
at `high` effort** (launched natively if available or through `scode-harness-shellout`, never hardcoding a launch
command here). Pass the executing checkout root, commit range or bookmark, and a private findings file in your scratch
directory, plus this full charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the supplied file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address all valid findings before moving on.

### Resource watchdog

Right after the resume protocol and before launching sub-agents, start the `scode-build-goal` skill's `scripts/watchdog`
(resolved via `realpath` next to that skill's `SKILL.md`), with status file
`farhelm-plan-triage-spec-and-comment-updates-watchdog.json` beside the working log, watching the executing checkout
root, `$HOME`, `/tmp`, the log's parent directory, and your scratch directory, at a 60-second interval with default
thresholds. Follow the `scode-build-goal` skill's Resource watchdog section for harness wiring, startup verification,
acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision, push all
consistent work and block per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things the
maintainer should know and possible follow-ups go into the delivery report.

### Done criterion

The draft PR exists on GitHub, satisfies all 9 acceptance criteria and the latest `## Decisions` entry (if any),
graduates its 9 `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed its `gpt-6.2-sol`
high review gate. Delivering the report is the executor's closing step (step 11 of `plans/.agents/executing.md`), not
part of the goal.

## Release note

No release note needed.
