# Dynamic network source for announcements

Written against main at fea806bc on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Dynamic network source for announcements.** add dynamic network source for announcements. we currently have hard
> coded announcements in releases, but i want one where i can push announcements by publishing them centrally and
> deliver to users. details tbd during planning.

In product terms, sharpened by the maintainer's planning decisions: add a centrally published, cryptographically signed
announcements feed on `https://get.farhelm.io/announcements.json` (+ `.minisig`) that the helm fetches on startup and
every 12 hours, verifies against Farhelm's compiled-in release signing keys, filters by optional version/OS/expiry
rules, and enqueues into the helm's existing durable announcement queue. When a fetch enqueues new announcements, the
helm also raises an app-wide notice in the sidebar's notification bell whose action opens the Announcements dialog (in
addition to the existing auto-open at next window start or reset). Users can disable the network check in Settings. A
path-only environment variable override on `https://get.farhelm.io` lets release brick testing verify the real
end-to-end flow at P2 against a standing signed test feed without relaxing TLS, redirects, or signing keys.

### Acceptance criteria

1. **Signed network feed & verification:** The helm fetches `https://get.farhelm.io/announcements.json` and
   `https://get.farhelm.io/announcements.json.minisig` (or a safe single-filename override path on `get.farhelm.io` from
   `FARHELM_ANNOUNCEMENTS_PATH`) over plain HTTPS with no redirects, bounded body/signature caps, and a bounded timeout,
   sending no user, machine, or version telemetry. Bytes are verified with `verify_signed_metadata` against
   `RELEASE_KEY_RING` using the fixed trusted comment `"farhelm announcements"` before any JSON is parsed. A 404 is
   quiet absence; transport, HTTP, size, signature, or schema failures log a warning and leave the queue untouched.
2. **Schema, targeting, link safety, and deduplication:** Version 1 of the feed JSON carries entries with the existing
   display fields (`id`, `title`, `body`, optional `detail`, optional `link { label, url }`) plus optional targeting
   filters: `min_version` (SemVer lower bound, inclusive), `max_version` (SemVer upper bound, inclusive), `os`
   (`"macos"` or `"linux"`, or a list of those), and `expires_at` (RFC 3339 timestamp; entries at or past expiry are
   ignored). Active entries apply to fresh installs as well as upgraded ones. Optional links must be
   `https://farhelm.io` with no custom port and no userinfo; any disallowed link is dropped while keeping the plain-text
   announcement. Entries are enqueued through `HelmStore::enqueue_announcements`, so already-shown IDs are never shown
   again and already-stored text is never replaced. Compiled-in release announcements continue working alongside the
   network feed.
3. **Cadence, dev-build gating, and Settings toggle:** The helm runs a check at startup and every 12 hours while
   running, skipping network fetches on development builds (`farhelm_proto::is_development_build`) unless tests inject a
   fetcher. `Preferences::check_announcements: Option<bool>` defaults to `true` when unset; when set to `false`, the
   helm makes no network announcement requests. A toggle in **Settings → Notifications** lets users on both macOS and
   Linux turn network announcement checks on or off (persisted in helm preferences and searchable in Settings).
4. **Sidebar notification bell integration:** When a network fetch inserts one or more new announcements into the queue,
   the helm raises (or re-raises as unread, with fresh `raised_at`) an app-wide notice
   (`NoticeContent::NewAnnouncement`) in the navigation rail's notification bell and pushes the update over `/api/ui`.
   The bell popover shows the notice with an action button (when unresolved) that closes the popover and opens the
   pending Announcements dialog, returning focus to the bell on close; dismissing the dialog records those exact shown
   IDs and resolves the bell notice once no pending announcements remain. Built-in startup release announcements do not
   raise this notice, and pending announcements still auto-open on the next window start or reset as today.
5. **Brick-test path override and P2 check:** Reading `FARHELM_ANNOUNCEMENTS_PATH` at the CLI/desktop entry points into
   `HelmArgs` (never calling `std::env` inside `network_announcements`) accepts only a simple single filename segment on
   `https://get.farhelm.io` (e.g. `announcements-test.json`); invalid values log a warning and are ignored rather than
   falling back to another host or disabling checks. The release brick-test procedure and harness
   (`releasing/mac-vm-test/`) add a P2 check (`p2-announcements`, same tier as `p2-feedback-send`, ahead of bonus
   historical-version checks) that launches Farhelm with `FARHELM_ANNOUNCEMENTS_PATH=announcements-test.json` and
   verifies fetch → signature verify → rail bell notice → clicking to open Announcements → dismissal →
   `? → Announcements` history.
6. **Specs and release docs updated:** `SPEC.md` ("Announcements", "Security", "Status", "Settings"), `SPEC_impl.md`,
   and `releasing/AGENTS.md` (documenting `announcements.json` + `.minisig` and trusted comment
   `"farhelm announcements"` alongside advisories; never authoring or running signing tooling) match the implemented
   behavior.

## Decisions already made

### Maintainer decisions (binding)

- **Hosting and trust:** Signed feed on `get.farhelm.io` (`https://get.farhelm.io/announcements.json` + `.minisig`),
  verified against Farhelm's compiled-in release signing keys. External publishing/signing tooling stays outside this
  repository; agents never author or run signing tooling.
- **Fetch cadence:** Helm checks on startup and every 12 hours (plain HTTPS GET, no redirects, no telemetry, skipped in
  development/test builds).
- **Display when newly fetched announcements arrive:** Keep waiting for the next window start or reset to auto-open the
  Announcements dialog, **and** raise a notification in the sidebar notification bell saying there is a new announcement
  whose click opens the Announcements window.
- **Targeting and fresh installs:** Optional `min_version`, `max_version`, `os` (`macos`/`linux`), and `expires_at`;
  active entries apply to fresh installs too; already-shown IDs never repeat.
- **Allowed link destinations:** `https://farhelm.io/...` only (disallowed links dropped, plain-text announcement kept).
- **Settings and compiled-in announcements:** On by default with a Settings toggle to turn it off; compiled-in release
  announcements stay alongside it.
- **Brick testing:** Path-only override env var on `https://get.farhelm.io` (keeping real origin, TLS, redirect policy,
  and compiled-in release signing keys) plus a P2 brick-test check (same tier as live feedback submission, ahead of
  bonus historical-version upgrade checks) added in this plan.
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** Verbatim maintainer wording in `## Release note` below.

### Binding repository constraints

- `AGENTS.md`, "The live install is off-limits" and "Testability and tests" (never mutate process env vars in tests;
  inject inputs).
- `releasing/AGENTS.md`: agents never author, run, or replace signing/publishing tooling or touch release keys.
- `crates/farhelm-helm/src/notices.rs`: adding a new app-wide notice kind requires a SQLite schema migration rebuilding
  the `notices` table while preserving all historical schema constants verbatim.
- `SPEC.md` ("Settings"): `Section::App` is listed only in a desktop app whose updater is active
  (`desktop::updates::runs`), so it is hidden on Linux and non-updater builds.
- `AGENTS.md`, "Tests and validation": changing `releasing/mac-vm-test/` scripts or checks must keep Python unit tests
  passing and run `python3 scripts/check-agents-md.py` if any instruction file or heading changes.

### Planner-chosen implementation shape (can adjust if simpler while keeping requirements)

- **Settings toggle placement:** Put `Choice::CheckAnnouncements` in `Section::Notifications`
  (`crates/gui/farhelm-gui/src/settings/choices.rs`), which is always listed on both macOS and Linux, preserves
  `SPEC.md`'s `Section::App` rule without churn, and sits next to notification controls.
- **Bell notice SQL helper:** Do NOT reuse `notices::reconcile_condition` as-is (it reopens resolved rows with
  `read = 1`, which would leave the bell silent for a second announcement if the user hadn't clicked "Clear resolved").
  Use a small dedicated helper in `notices.rs` operating on one `condition_key = ''` app-wide row that sets
  `resolved = 0, read = 0, raised_at = raised_now()` when `enqueue_announcements` inserts `> 0` new rows, and resolves
  it (`resolved = 1, read = 1`) in `announcements::shown` when pending count reaches 0, before bumping
  `manager.events()`.
- **HTTP fetch & URL validation:** Keep a small local bounded `bytes_stream` HTTPS-only no-redirect helper (~25 lines)
  and a local `https://farhelm.io/...` canonicalizer in `network_announcements.rs`, rather than generalizing
  `farhelm-desktop`'s updater/whats-new fetchers (wrong crate direction, follow redirects, or restrict path to
  `/docs/release-notes/#...`) or `release_payloads::fetch_capped` (follows redirects and formats provisioning errors).
- **Dialog open refactoring:** Split `record_on_dismiss: bool` from `after: Option<MainAction>` in
  `crates/gui/farhelm-gui/src/announcements.rs::open()` (~3 lines) so opening pending announcements from the rail bell
  records shown IDs on dismissal and returns focus to the bell without firing startup `after` callbacks. Hide the bell
  notice's action button when `notice.resolved == true` (matching `Retry now` in `rail_notices.rs`).
- **Feed limits & validation:** Cap body at 256 KiB, signature at 4 KiB, request timeout at 15 seconds, and validate
  `FARHELM_ANNOUNCEMENTS_PATH` against `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$` (no `/`, no `..`, no query/fragment).

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/announcements.rs` (`HelmStore::enqueue_announcements` returning inserted count, `shown`,
  `Collection::Announcements` push).
- `crates/farhelm-helm/src/provisioning/release_payloads.rs` (`RELEASE_KEY_RING`, `verify_signed_metadata`).
- `crates/farhelm-proto/src/version.rs` (`is_development_build`, re-exported by `farhelm-helm`).
- `crates/farhelm-proto/src/notices.rs`, `crates/farhelm-helm/src/notices.rs`,
  `crates/gui/farhelm-gui/src/rail_notices.rs`.
- `crates/farhelm-proto/src/ui_channel.rs` (`Preferences`), `crates/gui/farhelm-gui/src/settings/choices.rs`,
  `crates/gui/farhelm-gui/src/announcements.rs`.
- `crates/farhelm-desktop/src/bundle.rs` (`FARHELM_HELM_PAYLOAD_DIR` entry-point env-var pattern into `HelmArgs`).
- `releasing/mac-vm-test/brick_test.py`, `guest_workflow.py`, `guest_operations.py`, `BRICK-TEST.md`, `RECIPE.md`.

### PR stack

1. **`plan/dynamic-announcements/01-helm-feed-and-notice`** — Proto + helm backend:
   - Add `NoticeContent::NewAnnouncement` to `crates/farhelm-proto/src/notices.rs` and
     `Preferences::check_announcements: Option<bool>` to `crates/farhelm-proto/src/ui_channel.rs`.
   - Migrate `notices` table in `crates/farhelm-helm/src/notices.rs` and `store.rs` to admit `'new_announcement'` under
     `host_id IS NULL` (keeping all historical schema constants intact and updating schema migration tests).
   - Change `HelmStore::enqueue_announcements` to return `anyhow::Result<usize>` (inserted row count), add the dedicated
     `raise_new_announcement_notice` / `resolve_new_announcement_notice` helpers in `notices.rs`, and call
     `resolve_new_announcement_notice` inside `announcements::shown` when pending count reaches 0 before bumping events.
   - Add `crates/farhelm-helm/src/network_announcements.rs`: signed feed fetch, `verify_signed_metadata` with
     `"farhelm announcements"`, JSON v1 parsing, SemVer/OS/RFC 3339 expiry filtering, strict `https://farhelm.io/...`
     link sanitizer, startup + 12h background task wired in helm startup (skipping when `is_development_build` or
     `Preferences::check_announcements == Some(false)`), and `FARHELM_ANNOUNCEMENTS_PATH` plumbed via `HelmArgs` from
     CLI/desktop entry points.
   - Unit and integration tests with injected fetcher/clock/version/OS/keypair covering valid signed delivery,
     bad/wrong-purpose signatures, tampered bytes, 404 quiet absence, size/redirect rejection, targeting
     (`min_version`/`max_version`/`os`/ `expires_at`), fresh installs, link stripping, deduplication, Settings opt-out,
     and bell notice raise/re-raise/resolve.
2. **`plan/dynamic-announcements/02-gui-bell-and-settings`** — GUI bell notice and Settings switch:
   - Render `NoticeContent::NewAnnouncement` in `crates/gui/farhelm-gui/src/rail_notices.rs` with an action button
     (hidden when `notice.resolved`) that closes the popover and opens pending announcements with focus returning to the
     bell.
   - Decouple `record_on_dismiss` from `after` in `crates/gui/farhelm-gui/src/announcements.rs::open()`.
   - Add `Choice::CheckAnnouncements` under `Section::Notifications` in `crates/gui/farhelm-gui/src/settings/choices.rs`
     (and Settings search/render wiring), with unit tests and any affected golden screenshots refreshed and published.
3. **`plan/dynamic-announcements/03-docs-specs-and-brick-test`** — Specs, release docs, brick test, and TODO removal:
   - Update `SPEC.md` ("Announcements", "Security", "Status", "Settings"), `SPEC_impl.md`, and `releasing/AGENTS.md`
     (documenting `announcements.json` + `.minisig` format and `"farhelm announcements"` trusted comment beside
     advisories).
   - Update `releasing/mac-vm-test/BRICK-TEST.md`, `RECIPE.md`, `brick_test.py` (decoupling `is_feedback` from
     `priority == 'P2'` so P2 holds both `p2-feedback-send` and `p2-announcements`), `guest_workflow.py`,
     `guest_operations.py`, and their Python unit tests (`test_brick_test.py`, `test_native_coordinator.py`,
     `test_guest_operations.py`) to run the `FARHELM_ANNOUNCEMENTS_PATH=announcements-test.json` P2 check and verify the
     override is not left set afterward.
   - Remove the covered TODO entry from `TODO.md`.

## Fallbacks

- If `announcements::shown` finds that a pending announcement still exists (e.g. a second announcement arrived while the
  dialog was open), keep `NewAnnouncement` unresolved so the user still sees there is an unseen announcement.
- If a P2 brick-test run sees HTTP 404 on `announcements-test.json` (before the maintainer publishes the standing test
  feed), `p2-announcements` should fail/report clearly that `announcements-test.json` is not published yet without
  invalidating P0, P1, or `p2-feedback-send` (matching P2's non-blocking semantics in `BRICK-TEST.md`).
- Scope-reassessment triggers (examples, not a blacklist): adding a new HTTP client abstraction crate, caching signed
  feed files on disk outside SQLite, adding new UI chrome beyond the existing rail bell and Settings switch, or adding a
  new signing/key-override mechanism.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-dynamic-announcements-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-dynamic-announcements-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted checks per PR:

- PR 1: `farhelm-proto`, `farhelm-helm` (`announcements`, `network_announcements`, `notices`, `store`, `ui_channel`),
  `farhelm-desktop` + `farhelm` build/clippy (`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`), and `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`.
- PR 2: `farhelm-gui` (`rail_notices`, `announcements`, `settings`), `farhelm-desktop` clippy + tests,
  `python3 -B scripts/check-gui-lints.py`, `python3 -B scripts/check-crate-layering.py`,
  `python3 scripts/check-gui-goldens.py` (refresh and publish with `scripts/gui-goldens.py publish` if any Settings or
  rail-notices golden changed), and `check-test-sleeps.py`.
- PR 3: `releasing/mac-vm-test/` Python unit tests (`pytest` / `unittest` as `.agents/validation.md` lists for
  `releasing/mac-vm-test/`), `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and
  `python3 scripts/check-agents-md.py`.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Dynamic network source for announcements."), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

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

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, and
every review finding you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Starting in this release, in addition to automated update checking, there is an automated check for any urgent
announcements from upstream (once per 12 hours currently). You can turn it off in settings. This is meant to allow rare
announcements in case of e.g. critical bugs in the future. There has been no need so far to ever send such an
announcement. This is baking the feature in ahead of time so that if it ever is needed, the mechanism is there.
