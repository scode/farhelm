# Farhelm agent instructions

Intent and preferences only — for what the system is and how it works, read SPEC.md (product behavior) and SPEC_impl.md
(implementation choices and their motivations). Those documents are authoritative; do not restate them here and do not
change behavior they specify without surfacing the conflict. The historical build-order docs (PLAN.md / PLAN_M*.md,
cited by name throughout code comments) are archived under lore/ and are not maintained.

This project is either public now, or may become public in the future. No content in this project should contain
personal information such as personal usernames, hostnames, details about the local environments, etc.

## Reading these instructions

NOTE: Every agent gets this file without asking. It holds the rules for any task and a map of every flow; the procedures
live in the files it names. Codex injects only its first 32 KiB, so it stays under 28 KiB
(`scripts/check-agents-md.py`). It ends with `# End of AGENTS.md`; if that heading is not in your context, read this
file from disk.

- Start agents at the checkout root; from anywhere else, also read the `AGENTS.md` files between the root and there.
- A pointer (a "before ..., read X" or "when ..., read X" line, a "Flows" entry, an "Instructions by path" entry) is
  mandatory: read the file in full, in bounded chunks, checking for truncation notices (Codex can replace the middle of
  long output with `…N tokens truncated…`) and that you reached the end. A `Fence:` holds even if you never open the
  file.
- Read a flow's files once, before its first action, and a phase file on entering that phase. Re-read only after a
  compaction or resume, or when a file's text is no longer visible to you (a cleared tool result), and then every
  instruction file governing unfinished work, however you reached it. A summary saying a file was read is not its text,
  and re-reading instructions is not "duplicated work".
- After a compaction, find what governs unfinished work from "Flows" and "Instructions by path", listing changed paths
  from the repository, not the summary: `jj diff --summary -r 'main@origin..@'`, or
  `git diff --name-only origin/main...HEAD` plus `git status`. Read your working log first if your flow keeps one. At a
  drain's plan boundary or a landing round boundary, re-read the role's reading list and treat other instruction files
  read earlier as unread.
- A delegate gets only this file, or nothing if started outside a checkout: name the governing files in its prompt and
  tell it to read them (and this file, if needed) in full first.
- Detail files repeat some rules here word for word; change both copies together. Cite as `` `path`, "Heading" ``; old
  root headings resolve through `.agents/old-sections.md`. Never add a `CLAUDE.md`: it stops Claude Code from reading
  `AGENTS.md`.

## Unattended runs

When nobody is watching (a plan executor, the landing monitor, a delegate), an instruction in this repository's
instruction files or your goal file to get an answer, permission or approval from the maintainer or another person
before doing X means X waits for that answer. An answer already given counts: the request, the goal file, a
`## Decisions` entry, a recorded triage outcome. A check you settle from commands or evidence ("confirm the line still
carries your claim id") is not an ask; do it. If X is required, a claim holder (plan or landing claim) blocks per
`plans/AGENTS.md`, and any other delegate returns the question to its parent; otherwise skip X and report it as a
follow-up. An "ask first" from a skill or harness default is decided and logged unless it guards something this
repository fences (publication, merging, deletion). A required authorization never becomes a logged decision. Once
working, executors and the landing monitor never ask the maintainer anything in conversation, not even an optional
question, an offer of more work, or a harness question tool's prompt (`plans/AGENTS.md`, "Unattended means no
questions", names the one startup exception).

## The live install is off-limits

This machine may run the maintainer's own Farhelm, an ordinary installation that is installed and updated the way any
user's is. Do not assume anything about how it got there or where its pieces live beyond that. Take no state-changing
action against it: not its binaries, its state, or anything it runs (its helm, its supervisors, their service units or
processes, and the sessions in them). That means no running the installer or `farhelm helm setup` here, no rebuilding,
redeploying, or modifying its files, no start/stop/restart/reload/enable/disable/mask/kill of its units or processes,
and no edits to their unit definitions — the verbs are examples, not an exhaustive list, and "it is not literally named
above" is not a loophole. Not to verify a fix, not as a finishing step, and not because the README or release docs
describe how: install documentation is addressed to the human operator, and reading it is not authorization to run it.
`farhelm helm setup --dry-run` is read-only and allowed. The only exception is the user explicitly asking, in the
current session, for a specific one of these things to be done.

This rule is a fence with a history: twice (2026-08-12 and 2026-08-15), sessions asked only to merge PR stacks went on
to rebuild and restart the live helm, the second time deploying a build that locked the operator's browser out the next
morning. Nothing in normal development needs the live install — the test suites create their own supervisors and helms
from temporary state directories, and stopping a transient `farhelm-<uuid>-*.scope` unit that belongs to such a test is
fine.

## Sharing the machine with other agents

Several agents work on this project at once, on this machine, as the same unix user, each in its own checkout or jj
workspace. Do not reintroduce a fixed port, a fixed path under `/tmp` or `$HOME`, a fixed unit or container name, or a
fixed ssh alias in a harness; when a harness genuinely needs a stable name, derive it from the run's own random
directory. (Fixed lock files like `/tmp/fh-build.lock`, and the CentOS script's lock next to the ssh config, are the
exception the fixed-path rule allows: a lock exists to be shared, and it names no state.) `website/scripts/preview.sh`'s
fixed port is deliberate and is not a harness that the fixed-port rule applies to.

- One agent per checkout or workspace. Sibling checkouts and `jj workspace` directories are the supported shape; a
  shared `CARGO_TARGET_DIR` across workspaces is fine for cargo (it locks) but not for the dx and tmux-asset builds.
- Processes you did not start are someone else's. A helm or supervisor answering on a port you did not choose, a
  `farhelm-<uuid>-*.scope` unit whose UUID no run of yours minted, an `fh-it.*` or `fh-e2e.*` directory holding a live
  flock, or a docker container from the CentOS script: leave them alone. The one exception is
  `website/scripts/preview.sh` stopping another checkout's preview server when it takes its port
  (`website/.agents/previewing.md`).
- The `systemd --user` manager and the CPU are shared. A failure under that load is an environment event: keep the
  retained run, re-run narrowly when the machine is quieter, and do not log it as a flake or a regression on one
  observation.

The rest is in `.agents/machine.md`.

## Agent scratch space

Anything you create outside the checkout at your own discretion goes under `/tmp/farhelm-tmp/<id>/`, where `<id>` is the
output of one `uuidgen` you run the first time you need scratch space and reuse for the rest of the session. If you lose
track of it (after compaction, say), mint a new one rather than guessing which existing directory is yours, unless your
working log records it. Nothing bulky goes in `$HOME` (`~/.cache`, `~/.local`, the checkout's parent directory) or loose
in `/tmp`. Size does not matter: small files go there too. A `mktemp -d` that a skill asks for goes inside your
directory, not beside it. When you hand work to a delegate or subagent, give it a subdirectory of yours and tell it to
use that instead of minting its own. When the task that needed something bulky is over, delete it; keep what the task
still cites. Never delete another agent's directory.

Leave these where they already go: the checkout's own build output; shared caches of standard tools (use them the way
the tools do by default; do not redirect them); state created by the repo's harnesses and scripts (do not point `TMPDIR`
at your scratch directory when running tests or repo scripts); anything of your own that binds a unix socket (directly
under `/tmp` with a random name); anything another rule tells you to keep somewhere specific, such as goal files and
working logs.

When about to create anything outside the checkout, install a tool environment, allocate a build directory, use docker,
or clean up, read `.agents/machine.md` in full first.

## Claude Code shell commands

This section applies only to Claude Code sessions and their subagents. If you are Claude Code, a shell command you
compose and run (through your shell tool, or in a prompt to a Claude Code subagent that will run it) contains no glob
and no `$` expansion of any kind: no `*`, `?` or `[...]` patterns, no variables (`$HOME`, `$d`), no `$(...)` or
backticks, no `${...}`, no `$((...))`. Anything that needs one of those is written as a Deno TypeScript script, in a
file, and run as that file. Put it in your scratch directory unless it belongs in the repository. Plain commands with
literal arguments (`git status`) and pipelines of them stay fine in the shell. The exception is a command this
repository's instructions give word for word (the pinned nextest and tmux `PATH` setup, the release procedure's version
checks, the plans flow's ntfy send): run it as written, with only its placeholders filled in. Do not use the exception
to compose new commands in their style.

## Talking to the user

Assume the user does not know the codebase. They know Farhelm as a product and its core concepts (the helm, the
supervisor, sessions, hosts, and so on), but not the file names, functions, types, and other internal names behind them.
Explain things in those product terms. Do not use an internal name unless it has already come up in the conversation, or
you introduce it with enough context that it makes sense without opening the code.

## Conventional Commits

All commit messages and PR titles must use Conventional Commit format: `<type>: <short summary>`

Allowed types: `feat`, `fix`, `docs`, `perf`, `refactor`, `style`, `test`, `chore`, `ci`, `revert`.

Append `!` after the type for breaking changes (e.g. `feat!: remove legacy endpoint`). Scope is optional.

Rules:

- Type reflects the user-visible effect, not the implementation activity. A bug fix that requires heavy refactoring is
  `fix`, not `refactor`. A new CLI flag is `feat`, not `chore`.
- The summary after the colon is lowercase, imperative mood, no trailing period.
- Keep the first line under 72 characters.

## Changelog fragments

A PR whose title type is `feat`, `fix`, `perf`, `style`, or `revert`, or whose type carries `!`, adds a changelog
fragment under `releasing/changelog.d/` in the same commit as the change, without asking; the agent writes it. The file
name is a free mnemonic (`cursor-launch.md`), nothing parses it, and `README.md` there is not a fragment. The content is
a leading `---` front matter block holding a `kind:` line (`breaking`, `added`, `changed`, `fixed`, `removed`, or `none`
for a change with nothing user-facing, with the reason as its body), then a draft entry written for someone running
Farhelm: what changed for them, with caveats, and without internal mechanics, following the maintainer's wording rules
in `releasing/EDITORIAL_GUIDANCE.md`; its first paragraph is the entry candidate, and further paragraphs supply context
for curation. It is not reviewed at PR time; the release-time curation rewrites it.
`python3 releasing/check-changelog.py format` validates it. Other types add one only when the change has a user-facing
effect.

A `fix` or `feat` that amends work not yet released (a follow-up to an unreleased PR) edits that PR's fragment instead
of adding a second entry; the sweep counts an edited fragment as coverage. A `revert` of something that shipped in a
stable release is an entry; a revert of something that never shipped only removes the original's fragment. An optional
`pr: 123` (or `pr: 123, 456`) line claims PR numbers when the fragment is written after its change merged, or covers
several PRs at once; a fragment added in the change's own commit needs no `pr:`. Err toward including caveats, except
that the release cannot be downgraded from, which no entry mentions. When the draft rests on a guess (a PR with no
description, say), say so in the body so the curator verifies it.

## Frozen material

A repo-wide rename, sweep or cleanup leaves these alone; each changes only under a task that targets it:

- `lore/`: historical artifacts, decision records written when the decision was made, never updated to track code
  changes (`lore/AGENTS.md`).
- Published releases' `CHANGELOG.md` sections, `website/src/release-notes/*.json`, and the release shots and their tests
  are never touched by a refresh, a sweep, or a cleanup, only by a task that targets them.
- `TRIAGE_OUTCOMES.md` records, older `FLAKES.md` entries, and `plans/`, which has its own editing rules
  (`plans/AGENTS.md`).
- The captured agent screens under `crates/farhelm-supervisor/tests/fixtures/screens/`, which change only by re-capture
  (`docs/agent-screen-fixtures.md`).
- The README hero's and demo video's designs (scenarios, transcripts, `intent.md`) under `docs/readme-hero/` and
  `docs/readme-video/`, which change only when the maintainer asks (`.agents/docs-media.md`).
- The docs Overview page and the words in its intro SVGs, which mirror the README introduction (`website/AGENTS.md` has
  the rule and its one exception).

## Code rules

Everything Farhelm does differently per agent harness is answered per harness in a few named places, not by comparing
harnesses at the call site. The module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` map them, layer by
layer, and say where a new per-harness capability goes. Read that map before adding or changing harness-specific
behavior, add to the places it names, and do not write `kind == X`, `matches!` over specific harnesses, or a `_` arm
over harnesses in shared code: each of those hands a new harness whatever the branch does for the ones it does not name.

Read `docs/harness-marks.md` before adding or changing a harness mark (`crates/farhelm-ui/src/icons.rs` and the geometry
under `crates/farhelm-ui/src/icons/`); vendored geometry gets an entry in `THIRD_PARTY_NOTICES.md`.

## TODO.md and FLAKES.md

`TODO.md` is the maintainer's running list of wanted fixes and features. When a PR addresses an entry, remove that entry
in the same PR. Do not add entries on your own initiative. The exception is a deflake run (`deflake/AGENTS.md`), which
records the flakes it finds as Deflake entries. When a Deflake entry is removed, remove the matching line in
`deflake/known-flakes.txt` in the same PR.

Before adding or revising an entry, and before answering any question about what TODO.md currently holds (a "tldr todo"
included), fetch and rebase the working copy onto the latest main per jjstack: `jj git fetch`, then rebase the
working-copy change onto `main@origin`. In a stack (a deflake run, plan or triage execution), fetch and read main's
`TODO.md` instead of rebasing mid-run, and keep both sides when the stack's own rebase meets a `TODO.md` conflict.

`FLAKES.md` is the append-only log of LATENT flakes: tests that pass where they were written and fail elsewhere,
sometimes, or under load, whose cause was not obvious when they failed. When you establish that a failure is a true
flake of that kind — pre-existing, or one that survived past the session that wrote it — add a dated entry. Do NOT log a
test that was written, flaked, and was fixed within the same session. Open flakes also get a deflake entry in TODO.md.
Before writing an entry, read `.agents/test-evidence.md`, "FLAKES.md", for its fields.

## Tests and validation

For changes to tests or their fixtures, apply `.agents/test-authoring.md` during authoring and supply its full text
verbatim to the independent reviewer. Use the review process selected for the task; this checklist does not request an
additional reviewer or a review swarm. Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter
prepared in `docs/test-sleep-check.md` when changing Rust or browser tests, their helpers, or the checker.

Before creating, updating, or merging a PR, or claiming work is done, make a judgment call about what validation would
add useful evidence. Never mechanically run the entire test battery. A PR, rebase, stack completion, or merge request is
not by itself a reason to launch it. For a documentation-only change that cannot affect generated artifacts or
executable examples — ordinary Markdown documentation, a lore entry, agent instructions, or comments with no doctest or
generation role — do NOT run Rust, JavaScript, desktop, installer, provisioning, or browser tests. Run only formatters,
linters, link checkers, or other checks that actually inspect the changed files, if any exist. Reuse is per command.
Failed, interrupted, poorly identified runs, or runs whose coverage no longer applies are not evidence of a pass. Report
checks run now; checks reused, with the covered revision and why their coverage still applies; and checks skipped, with
the reason.

Run local and worker runtime tests through `python3 scripts/record-test-run.py`, including each reproduction attempt.
Runtime `SKIPPED` messages still require reading the retained output: an early-return test is a JUnit pass, not proof
that its required systemd or SSH substrate ran. Keep the failed UUID run directory before retrying, and record its path
and disposition in the session's working log. A later pass is another observation, not a replacement for the failure.
When investigating a failing, flaky, or suspicious test, reproduce it with the narrowest run that could show it, and
widen only when the narrow run will not reproduce; read `.agents/narrow-tests.md` first.

The CI workflow runs ONLY on demand: it has no push or pull-request trigger (removed 2026-09-12), so neither a PR, a
`gh pr ready`, nor a merge to main starts a run. Browser end-to-end validation's CI job is disabled (`if: false` in
ci.yml), so CI green does not include browser coverage. PRs are still opened as drafts and marked ready when the user
asks to publish them, or as part of landing them, never on the agent's own initiative. Landing is authorized where a
flow's own file says so: plan landing, approved planning PRs, abandoned-plan source updates, release fix forward, the
stable release's changelog and finalization PRs, a stable re-cut's changelog PR, run-all-tests fixes, the todo flow.

- Before choosing validation, running any runtime test (Rust, browser, JS, installer, provisioning), creating or
  updating a PR, merging one, or reporting a change to the repository's files as done, read `.agents/validation.md` and
  `.agents/test-evidence.md` in full (the inventory, the pinned nextest and tmux setup, the recorder's arguments, the
  browser engines).
- When changing either workflow (`ci.yml`, or the release workflow through `release.yml`, `dist-workspace.toml` or
  `.github/dist-build-setup.yml`), a check or test script, or the deflake sweep's phases, update the inventory in
  `.agents/validation.md` and the lists in `.agents/run-all-tests.md` in the same change.
- Any change to an instruction file, or one that renames or removes a Markdown heading or moves or renames a file, runs
  `python3 scripts/check-agents-md.py`.

## Instructions by path

Before creating, editing or deleting anything under these paths, read the file named:

- `plans/`: `plans/AGENTS.md`.
- `releasing/` (except fragment files under `releasing/changelog.d/`), `CHANGELOG.md`, the version in `Cargo.toml` or
  `packaging/farhelm-desktop/dist.toml`, `.github/workflows/release.yml` (generated: never hand-edit it),
  `dist-workspace.toml`, `.github/dist-build-setup.yml`: `releasing/AGENTS.md`.
- `review_feedback_queue/`, `TRIAGE_OUTCOMES.md`: `review_feedback_queue/AGENTS.md`.
- `website/`, `README.md`: `website/AGENTS.md`.
- `deflake/`: `deflake/AGENTS.md`.
- `lore/`: `lore/AGENTS.md`.
- `crates/farhelm-ui/src/icons.rs`, `crates/farhelm-ui/src/icons/`: `docs/harness-marks.md`.
- `crates/farhelm-supervisor/tests/fixtures/screens/`: `docs/agent-screen-fixtures.md`.
- `packaging/farhelm-desktop/`, `website/src/assets/intro/`, `docs/readme-hero/`, `docs/readme-video/`,
  `docs/docs-shots/`, `e2e/readme-hero/`, `e2e/readme-video/`, `e2e/docs-shots/`, `scripts/readme-screenshot.sh`,
  `scripts/publish-readme-hero.sh`, `scripts/readme-video.sh`, `scripts/docs-screenshots.sh`,
  `scripts/publish-docs-shots.sh`: `.agents/docs-media.md`.
- `.github/workflows/`, `scripts/`, `e2e/`, any test: `.agents/validation.md`.

## Flows

Each entry: trigger phrases, the file to read in full before the first action, and its fence.

- **Release brick test** ("brick test release vX.Y.Z", "stop after P0 checks", "stop after P1 checks", "did P0 checks
  succeed?", "did P1 checks succeed?"): `releasing/mac-vm-test/BRICK-TEST.md`. Fence: a test request does not authorize
  publication or release promotion; an OS, computer-use or authentication permission block during a test is a harness
  bug: stop, retain the failure, and do not manually approve or switch to host UI to rescue it; do not interrupt an
  active operator, delete retained data, force-stop guests or touch unrelated VMs; `/latest` still naming the preceding
  promoted stable is expected, not a blocker.
- **Releases** (stable, RC or dev releases, fixing forward, advisories, changelog curation): `releasing/AGENTS.md`.
  Fence: no release tag before its base and version are settled with the maintainer, which happens when the release is
  requested, never by stopping later; never reuse or delete a tag; agents never author, run or replace the signing and
  publication step, publish an advisory file, or move `/latest`; no release bump reaches main.
- **Careful rebase**: `.agents/careful-rebase.md`. Fence: a rebase that applies cleanly can still be wrong; a conflict
  that needs a design decision is the user's (unattended: a block).
- **Vercel** ("trigger vercel on main", "deploy the website", "trigger vercel on this PR"): `.agents/vercel.md`. Fence:
  deploy only when asked; the script builds from GitHub, not the working copy.
- **devbuild** ("devbuild <thing to test>", or any other request to try a PR or branch on the maintainer's Mac, or to
  run, test or see it there): `.agents/devbuild.md`. Fence: say so when the target changes a database schema.
- **TODO board and todo flow** ("tldr todo", "what's in the todo", a message starting with "todo flow,"):
  `.agents/todo.md`. Fence: the implied merge applies ONLY while TODO.md is the only file the session has changed;
  otherwise commit or merge nothing automatically, say so and ask.
- **Deflake run** ("start a deflake run"): `deflake/AGENTS.md`. Fence: never poll; act only when
  `deflake/bin/deflake wait` returns.
- **Run all tests and fix** (and similar wording asking for every test to be run and the failures fixed):
  `.agents/run-all-tests.md`. Fence: an explicit request, never a finishing step; land straightforward fixes when their
  checks pass, and record non-trivial or scope-expanding work for a decision.
- **Planning system** ("todo planning flow", "todo planning flow for <bucket>", "plan to implement <TODO items>", "use
  the planning system to schedule these", "pick a plan to execute", "execute the next plan", "execute the next plan in
  order", "execute plan X", "drain the plans", "drain the plans and keep monitoring", "monitor for complete plans",
  "land the complete plans", "review the plans", "show blocked plans", "check for blocked plans", "release plan X",
  "plan help"): `plans/AGENTS.md`. Fence: every planning flow starts on a clean working copy of the latest `main`, after
  reconciling any recorded claim of its own (a dirty working copy belonging to that claim is resumed or reset, not
  reported); any other dirty working copy aborts the flow with a report; see also "Unattended runs".
- **Review feedback** (recording it, commissioning a review that feeds the queue, "perform trailing code review",
  "triage review feedback", "execute triage outcomes"): `review_feedback_queue/AGENTS.md`. Fence: triage is persistent;
  "fix this", "yes", "agreed" during it are decisions to record, not permission to edit; execution needs a separate
  command that names it. When asked to address a queued item, remove the item (or narrow it, if partially addressed) in
  the same commit, change, or PR as the fix.
- **Docs website** (writing or changing docs pages, "refresh the docs"): `website/AGENTS.md`. Fence: when the task's
  main work changes the docs, start the preview and hand over page links when reporting, without being asked; unattended
  runs and incidental docs edits start no preview server and name the changed page instead.
- **Screenshots, demo video, brand marks** ("refresh the README screenshot", "refresh the demo video", "refresh the docs
  screenshots", anything showing Farhelm's icon or name as a mark): `.agents/docs-media.md`. Fence: only the publish
  scripts push the screenshot refs; never commit a captured screenshot on main; never hand-edit the wordmark SVGs.
- **Agent screens** ("re-capture agent screens", checking a new claude or codex against status detection):
  `docs/agent-screen-fixtures.md`. Fence: it spends real vendor turns; only when asked or after an agent upgrade.
- **Desktop or web UI bug reports**: before investigating one, read `docs/desktop-web-triage.md`.
- **Agent TUI behavior**: before relying on a claim about how an installed agent behaves in a terminal, read
  `.agents/real-agent-tui.md`.

# End of AGENTS.md
