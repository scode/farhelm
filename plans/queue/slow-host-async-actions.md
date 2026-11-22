# Stop slow hosts from blocking unrelated desktop actions (Plan 1: two-lane UI channel and top 3)

Written against main at 10973c17e on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan is the first wave for one TODO.md entry (bucket "Near term"). Verbatim:

> **One slow host holds up every other action for up to 20 seconds.** The desktop app sends its commands to the helm one
> at a time, each starting when the previous one has answered. Saving a replacement credential (delivered to every host,
> at most 20 seconds per host) and deleting from Trash (at most 20 seconds per host) can therefore keep every later
> click that needs the helm (a launch, a Stop, another save) waiting for up to 20 seconds when one host is slow or
> unreachable. The window itself stays responsive, but actions look stuck. Check whether the per-host waits run in
> parallel (if they run one after another, the worst case is 20 seconds times the number of hosts), and improve this,
> for example by moving these routes to the accepted form, as provisioning already is.

In product terms: today, every click in the desktop app that talks to the helm runs strictly one at a time over the UI
channel, and live UI pushes pause while any command is in flight. A single slow host (or a healthy Tensorlake Resume,
which routinely takes 20–120 seconds) freezes every unrelated click and pauses live status updates for up to 20 seconds;
multi-host actions like opening Trash, Trash "Delete all", and bulk Stop/Delete stack 20 seconds per slow host.

This plan builds the shared two-lane UI channel mechanism, unblocks live UI pushes, and moves the **top 3** real-world
stalls onto the unordered lane: (1) environment Suspend, Resume, Stop-and-suspend, environment Delete (session menu) and
Retry-delete; (2) Checkout Trash listing and deleting; and (3) session Stop and ordinary Delete. Remaining slow routes
(credential replacement, launcher reads, session Create/Rename/Restart/Replace/Tabs/Tunnels, and host management
including Hosts-panel Delete sandbox/container) are intentionally deferred so the maintainer can review the resulting
architecture before converting more routes.

This plan depends on `launch-wakes-suspended-host.md` landing on `main` first (`INDEX.md` carries
`(after launch-wakes-suspended-host.md)`), so you can check directly whether that plan assumed a host's phase arrives
only with its command's answer.

### Where it stands (from planning research, a fresh-context Opus design consult, and planning review at main 10973c17e; line numbers approximate)

- **No `PROTOCOL_VERSION` bump exists for the UI channel.** `crates/farhelm-proto/src/lib.rs` documents that
  `PROTOCOL_VERSION` versions only the helm↔supervisor edge; the desktop app embeds its helm so the two are always the
  same build. Adding a defaulted field to `Command` in `crates/farhelm-proto/src/ui_channel.rs` needs no version bump.
- **Every helm route already handles concurrent calls safely.** The CLI, agents, and HTTP callers already hit these
  routes concurrently. Serial admission in `crates/farhelm-helm/src/ui_channel.rs` (about 11-23, 997-1072) exists only
  for the desktop app's own causal assumptions (such as a launch not overtaking a template save). Therefore the desktop
  app—not a duplicated helm URL table—owns the lane choice.
- **Helm UI channel (`crates/farhelm-helm/src/ui_channel.rs`).** `run()` holds a single `in_flight: Option<InFlight>`
  and a FIFO `queue: VecDeque<Command>` capped at `MAX_QUEUED_COMMANDS` (about 130, 1009-1072). Line 1059 guards live
  pushes with `changed = feed.changed(), if in_flight.is_none() =>`, freezing all live pushes while any command runs.
  Each completion flushes `feed.borrow_and_update()`, `channel.changes().await`, and writes all `Changes` followed by
  `Reply` on `outgoing`. `SESSION_QUERY_PATH` mutates channel-local state and must stay ordered.
- **Helm client (`crates/farhelm-helm-client/src/channel.rs` and `fake.rs`).** `channel.rs` (about 418-512) keeps
  `in_flight: VecDeque<InFlight>`, breaks the channel on any out-of-order reply, and tracks a single 30-second watchdog
  from `admitted`. `fake.rs` (about 59, 137-162) asserts FIFO in `FakeHelm::reply_marked`.
- **Desktop GUI (`crates/gui/farhelm-gui/src/`).** `UiSession` (`session.rs`, about 104) already tracks arbitrary
  concurrent replies (`HashMap<u64, Task<()>>`). `Operations` (`operations.rs`) already enforces per-resource exclusion
  via `OpKey` (including bulk holds) and defines `Work` with exhaustive one-arm-per-line decision methods
  (`creates_a_session`, `changes_host_registry`, about 230-235 — follow that append-only one-arm-per-line style for
  `Work::lane()`). Note the exact `Work` variants: `Work::Delete(DeleteGuard)` (about 96) is ordinary session Delete,
  `Work::EnvironmentDelete` (about 216, built in `session_list/environment_delete.rs`) covers session-menu
  environment-session and whole-environment Delete, `Work::RetryHost` covers both fast host Retry and slow Retry-delete,
  and Hosts-panel Delete sandbox/container uses `Work::RemoveHost` (`hosts/remove.rs`, about 331, shared with SSH host
  Remove and `changes_host_registry() == true`), which stays `Ordered` in Plan 1. Keyless Trash listing goes through
  `operations::send`.
- **Reordering hazard found during planning (`trash.rs` / `trash/listing.rs`).** In `TrashButton::deleted`,
  `listing::accepted_cleanup` currently applies to whatever listing is stored when the delete reply arrives. With live
  pushes unblocked, an epoch-triggered read sent _after_ the delete can answer _before_ the delete's reply; applying
  `accepted_cleanup` to that fresher listing sets `state.removing = true` without scheduling a recheck.
- **Stale comments that become false once live pushes unblock:** `crates/gui/farhelm-gui/src/hosts/lifecycle.rs` (about
  36), `crates/gui/farhelm-gui/src/session_list/environment.rs` (about 63), and
  `crates/gui/farhelm-gui/src/session_pane/tests.rs` (about 1557,
  `a_suspend_from_this_window_holds_back_the_reconnect_card`) claim the window receives no pushes while its own command
  runs; `trash/listing.rs`, `trash/dialog/tests.rs` (about 373), and `settings/credentials.rs` cite the old single-lane
  FIFO rule.
- **Credential replacement inaccuracy in TODO.md:** `crates/farhelm-helm/src/credentials.rs` (about 300) already uses a
  single shared 20-second `HOST_REPLY_BOUND` across hosts pushed in parallel (`PUSH_CONCURRENCY = 4`), not 20 seconds
  per host.

### Acceptance criteria

1. **Unblock live UI pushes (step 1).** Remove `if in_flight.is_none()` from `feed.changed()` in
   `crates/farhelm-helm/src/ui_channel.rs`, keeping it as the lowest-priority arm of the `biased` `tokio::select!`.
   Preserve the invariant that each command completion writes its `Changes` and its `Reply` contiguously with nothing in
   between. Update the comments in `hosts/lifecycle.rs`, `session_list/environment.rs`, and `session_pane/tests.rs` that
   claimed pushes pause while a command runs. Check whether any existing `ui_channel_tests.rs` test asserts the pause
   (if one does, update it and log a DECISION).
2. **One defaulted wire field: `Command.lane`.** In `crates/farhelm-proto/src/ui_channel.rs`, add
   `pub enum Lane { #[default] Ordered, Unordered }` and
   `#[serde(default, skip_serializing_if = "Lane::is_ordered")] pub lane: Lane` on `Command`. No other wire changes, no
   new `ClientMessage`/`ServerMessage` variants, no `PROTOCOL_VERSION` bump.
3. **Helm two-lane execution (`crates/farhelm-helm/src/ui_channel.rs`).**
   - `Ordered` commands keep the exact existing FIFO `queue` and `ordered_in_flight: Option<InFlight>` semantics.
   - `Unordered` commands start immediately in a `FuturesUnordered<InFlight>` owned directly by `run()` (not detached
     `tokio::spawn` tasks, so dropping `run()` cancels held cancellable futures on disconnect/displacement). Guard the
     select arm with `if !unordered.is_empty()`.
   - Channel-local `SESSION_QUERY_PATH` ALWAYS executes on the `Ordered` lane regardless of its `lane` field.
   - One shared cap: `ordered_queue.len() + unordered.len() >= MAX_QUEUED_COMMANDS` closes the channel (no per-lane caps
     or GUI throttling).
   - Both lanes flush `feed.borrow_and_update()` + `channel.changes().await` and emit `Changes` followed immediately by
     `Reply` contiguously.
4. **Client per-lane matching and exact watchdog (`crates/farhelm-helm-client/src/channel.rs` and `fake.rs`).**
   - Keep `ordered: VecDeque<InFlight>` and `admitted` untouched for ordered commands; add
     `unordered: BTreeMap<u64, (Completion, Instant)>` recording each unordered command's send time.
   - On `Reply(reply)`: if `reply.id` matches `ordered.front()`, pop it and update `admitted` as today; else remove
     `reply.id` from `unordered`; else break the channel as a protocol violation (unknown id, duplicate id, or ordered
     reply out of ordered order).
   - Watchdog deadline while either structure is non-empty:
     `min(admitted if !ordered.is_empty(), first_unordered_sent if !unordered.is_empty()) + self.config.watchdog`.
   - On reset/disconnect, fail every command in both `ordered` and `unordered` with `Reset` _after_ announcing the state
     change.
   - `FakeHelm`: keep strict FIFO enforcement on ordered commands, allow answering unordered commands in any order, and
     expose `FakeCommand.lane`.
5. **Fixed unordered set in the desktop app (`crates/gui/farhelm-gui/src/`).**
   - Add an exhaustive one-arm-per-line `Work::lane(&self) -> Lane` match in `operations.rs` with the documented safety
     rule: a work kind may be `Unordered` only if (a) nothing the GUI sends later depends on its effect before its reply
     other than through its own `OpKey`, and (b) it does not depend on the effect of an earlier unanswered command other
     than through its own `OpKey`.
   - Mark **only** this fixed set `Unordered`: `Work::Stop`, `Work::Delete(_)` (ordinary session Delete),
     `Work::EnvironmentDelete` (session-menu environment-session and sandbox Delete), `Work::Suspend`, `Work::Resume`,
     `Work::StopAndSuspend`, `Work::TrashDelete`, `Work::RetryHost` (kept unsplit, covering both Retry and
     Retry-delete), plus one sibling helper (`operations::send_unordered` or internal `Lane` param) used only for the
     keyless Checkout Trash listing (`POST /api/checkout-trash/list`). Every other `Work` variant (including
     `Work::RemoveHost`, shared with SSH host Remove and Hosts-panel Delete sandbox/container) and `send`/`read` call
     stays `Ordered`. You may _defer_ an item from this list if it proves unexpectedly tangled (record why in the log
     and follow-up TODO); never _add_ one.
   - Apply the pre-decided Trash guard in `TrashButton::deleted`: skip `listing::accepted_cleanup` when `read_since` is
     true. Keep the two existing Trash `DECISION` behaviors (no recheck on failed read, no read after 504 delete) as-is,
     correcting comments that still claim channel blocking.
   - Verify compatibility with the landed `launch-wakes-suspended-host` code.
   - Change NO helm route handlers, NO `HOST_REPLY_BOUND`, NO watchdog duration, NO 504 wording, and perform NO
     `202 Accepted` conversions.
6. **Tests, without sleeps (`.agents/test-authoring.md`):**
   - **Helm (`ui_channel_tests.rs`, gates instead of sleeps):** unordered answers while ordered is held; two ordered
     stay FIFO while unordered completes between them; feed bump pushes while a command is held; unordered `Changes`
     precede its `Reply`; dropping the channel drops a held cancellable unordered future; cap counts both lanes;
     unordered-marked `SESSION_QUERY_PATH` stays ordered.
   - **Client (`channel/tests.rs`, paused tokio time):** unordered reply may overtake; ordered reply out of ordered
     order still breaks the channel; unknown or duplicate id breaks the channel; unordered clock runs from send and
     ordered clock from admission; an ordered command queued behind a slow unordered one is not timed out early; reset
     fails both maps after state announcement.
   - **Fake:** unordered answered in any order; ordered still enforces FIFO; `FakeCommand.lane` exposed.
   - **GUI (fake):** each converted work kind and keyless Trash list sends `Unordered`, and a representative ordered one
     (e.g. `Rename`) sends `Ordered`; the Trash read-after-delete guard (`read_since` skips `accepted_cleanup`); a
     launch whose new session row is pushed before its reply still behaves identically.
7. **Docs & spec updates:**
   - `SPEC_impl.md`, "The UI channel" and "One failure path": record the declared exception (ordered by default;
     commands the desktop app marks `Unordered` run concurrently) and per-lane watchdog admission.
   - Module docs in `farhelm-proto/src/ui_channel.rs`, `farhelm-helm/src/ui_channel.rs`,
     `farhelm-helm-client/src/channel.rs` and `fake.rs`.
   - Fix false comments in `hosts/lifecycle.rs`, `session_list/environment.rs`, `session_pane/tests.rs`,
     `trash/listing.rs`, `trash/dialog/tests.rs`, and `settings/credentials.rs` (update cited reason to "ordered
     commands" where still true). Grep for "one at a time" and "ordered channel" and fix only claims that became false.
8. **TODO.md updates and changelog fragment in the last PR:**
   - **Narrow (do NOT remove)** the existing `One slow host holds up every other action for up to 20 seconds.` entry in
     `TODO.md` (`Near term`): record that Plan 1 moved the UI channel to two lanes, unblocked live pushes, and converted
     environment Suspend/Resume/Stop-and-suspend/Delete/Retry-delete, Trash list/delete, and session Stop/Delete; fix
     the inaccurate "at most 20 seconds per host" claim for credentials to "at most 20 seconds total across hosts"; and
     list the remaining unconverted routes (credential replacement first, then launcher reads, Launch, session
     Rename/Restart/Replace/Tabs/Tunnels, and host management/settings including Hosts-panel Delete sandbox/container).
   - **Add a mandatory entry** in `TODO.md` under `## Near term` → `### Plan follow-up` (per `plans/AGENTS.md`,
     "Follow-up TODO entries"): review the resulting two-lane UI channel architecture, watchdog behavior, `Work::lane()`
     boundaries, and any deferred or noticed edge cases (including the Trash `2N` connect+open burst note and the two
     deferred Trash `DECISION` behaviors) before converting any further routes.
   - Leave a changelog fragment (`kind: fixed` or `kind: changed`) describing what users see in plain terms.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page (after a fresh-context Opus design consult):

- **Client-declared `Command.lane` field approved:** Add `lane: Ordered | Unordered` (default `Ordered`) to `Command` in
  `farhelm-proto/src/ui_channel.rs`, chosen by the desktop app in `Work::lane()`. Accepted the `SPEC_impl.md` declared
  exception to "one ordered stream" and the three minor visible behavior differences (immediate "busy" on conflicting
  environment presses; launch sent right behind container Resume no longer waiting behind it; briefly seeing both rows
  during Replace).
- **Drop the `202 Accepted` escape hatch from Plan 1:** Pure deferral (leaving a messy item out of the unordered set and
  recording why) is the only escape hatch in Plan 1.
- **Top 3 fixed for Plan 1:** (1) Environment Suspend/Resume/Stop-and-suspend + session-menu environment Delete &
  Retry-delete; (2) Checkout Trash list & delete; (3) Session Stop & ordinary Delete. Credential replacement and other
  routes (including Hosts-panel `Work::RemoveHost`) stay deferred until after the mandatory architecture review.
- **Production code ceiling for Opus design-review gate: 2,000 lines.** The maintainer explicitly raised Opus's 300-line
  trigger to 2,000 lines ("the fundamental complexity gate is what's important; I don't want churn due to line counts if
  the fundamental design is solid and we'll be reviewing for simplicity later"). Keep the fundamental complexity
  triggers strict.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the request for an Opus-reviewed up-front design with strict
anti-complexity design-review gates, the Top 3 wave + mandatory architecture review follow-up, and the decisions above.

**Binding repository constraints:**

- `SPEC.md` ("Losing the connection to the helm", "Slow hosts") and `SPEC_impl.md` ("The UI channel", "One failure
  path").
- Root `AGENTS.md`, "Code rules" (keep Tensorlake and Podman environment lifecycle uniform — both share `Work::Suspend`,
  `Work::Resume`, `Work::StopAndSuspend`, `Work::EnvironmentDelete`).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- `plans/AGENTS.md`, "Follow-up TODO entries": new follow-up TODO entries go in `## Near term` → `### Plan follow-up`.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research, the fresh-context Opus consult, and the independent planning review):

- Split into three bite-sized PRs: (1) unblock live pushes + fix stale comments + Launch intermediate-push test; (2)
  `Command.lane` wire field + helm two-lane loop + client/fake per-lane matching and watchdog; (3) GUI `Work::lane()`,
  keyless Trash list, Trash `read_since` guard, spec/docs updates, changelog fragment, and TODO updates.
- Keep `Work::RetryHost` unsplit (covering both fast host Retry and slow Retry-delete) rather than splitting the variant
  across `operations.rs`.
- Accept the theoretical `2N` connect+open Trash dialog burst (>32 slow hosts) for Plan 1 and record it in the
  architecture-review follow-up TODO rather than inventing per-lane caps or GUI throttling.

## Outline

1. `plan/slow-host-async-actions/01-unblock-ui-channel-pushes`: remove `if in_flight.is_none()` from `feed.changed()` in
   `ui_channel.rs` (keeping it lowest priority in the `biased` select and preserving contiguous `Changes`-then-`Reply`
   batches); add the helm test that a feed bump pushes while a command is held; fix the stale comments in
   `hosts/lifecycle.rs`, `session_list/environment.rs`, and `session_pane/tests.rs`; add the GUI fake test that Launch
   handles its new row pushed before its reply. `fix: keep live UI updates flowing while a host command waits`, with a
   changelog fragment (`kind: fixed`).
2. `plan/slow-host-async-actions/02-two-lane-ui-channel`: add `Lane` (`Ordered` default, `Unordered`) to `Command` in
   `farhelm-proto/src/ui_channel.rs`; implement the helm's `FuturesUnordered` unordered lane in `ui_channel.rs` (shared
   cap, `SESSION_QUERY_PATH` always ordered, drop-on-disconnect); implement client per-lane reply matching and exact
   per-lane watchdog in `farhelm-helm-client/src/channel.rs`; update `FakeHelm` to enforce ordered FIFO while allowing
   unordered answers and exposing `FakeCommand.lane`; add all helm, client, and fake tests above; update `SPEC_impl.md`
   and module docs. `refactor: add unordered lane to the UI channel`.
3. `plan/slow-host-async-actions/03-unordered-top3-actions`: wire `Work::lane()` (one arm per line) in `operations.rs`
   for the fixed unordered set (`Stop`, `Delete(_)`, `EnvironmentDelete`, `Suspend`, `Resume`, `StopAndSuspend`,
   `TrashDelete`, `RetryHost`) plus `send_unordered` for keyless Trash listing; add the `read_since` guard in
   `TrashButton::deleted`; verify compatibility with landed `launch-wakes-suspended-host`; fix remaining false comments
   (`trash/listing.rs`, `trash/dialog/tests.rs`, `settings/credentials.rs`, grep audit); add GUI fake tests for lane
   assignment and the Trash race guard; amend PR 1's changelog fragment to cover the full user-visible improvement;
   narrow the original TODO entry and add the mandatory `Plan follow-up` architecture-review TODO.
   `fix: stop slow hosts from blocking unrelated desktop actions`.

## Opus design-review gate & Fallbacks

**Mandatory Opus design-review gate:** Stop and consult a fresh-context **Opus (`high` effort)** agent (via
`$scode-harness-shellout` if not native) before writing code if ANY of the following triggers occur — do NOT improvise
around them:

1. You feel pressure to add any wire change beyond `Command.lane`, any new `ClientMessage`/`ServerMessage` variant, a
   `Started` ack, per-resource ordering keys, cross-lane causal barriers/fences, or a server-side timeout.
2. You feel pressure to change ordered-lane semantics or relax an existing ordered-lane test assertion (other than a
   test asserting the feed pause that PR 1 intentionally removes).
3. Any GUI feature appears to need new ordering/latest-wins/queueing state beyond the pre-decided 3-line Trash
   `read_since` guard.
4. You want to add anything outside the fixed unordered list, convert any route to `202 Accepted`, or modify any helm
   route handler, `HOST_REPLY_BOUND`, watchdog duration, or 504 wording.
5. Total production code added/changed across the stack (excluding docs and tests) exceeds **2,000 lines**.
6. A new test appears to need wall-clock sleeps instead of paused tokio time or gates, or flakes even once.
7. An intermediate push (from PR 1) breaks a GUI flow and the fix is not a one-line guard.
8. Landed `launch-wakes-suspended-host` relies on the old "host phase arrives only with the command's reply" behavior.

If the Opus consult recommends a design departure that weakens a guarantee or expands scope beyond the maintainer's
settled decisions, block the plan per `plans/.agents/executing.md`, "Executing one plan" (step 10).

**Routine fallbacks:**

- You may _defer_ any individual item from the fixed unordered list if it hits an unexpected causal dependency: leave it
  `Ordered`, log a `DECISION` with the concrete reason, and list it in the narrowed TODO entry and the follow-up review
  TODO. Never _add_ a route outside the list.
- If combining PR 2 and PR 3 is clearly cleaner without growing the diff, you may merge them into one PR and log it as a
  DECISION; keep PR 1 separate so unblocking pushes is independently reviewable.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-slow-host-async-actions-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-slow-host-async-actions-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes (for example `farhelm-proto`, `farhelm-helm`
`ui_channel_tests`, `farhelm-helm-client` `channel::tests` and `fake`, and `farhelm-gui` unit tests for `operations`,
`trash`, `hosts::lifecycle`, `session_list`, `session_pane`, and `launcher`, plus `cargo fmt --all -- --check` and
`cargo clippy --all-targets -- -D warnings`). Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

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
or Opus design-review consult, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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
- The last PR of the stack narrows (does NOT remove) the TODO.md entry this plan covers and adds the mandatory
  `Plan follow-up` architecture-review TODO entry, per acceptance criterion 8.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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

- before implementing a substantial departure from the outline above (the plan-specific triggers are under Opus
  design-review gate above, which require Opus specifically; this section covers all other departures), including a new
  mechanism that does the job of an existing one or a generalization the outline did not plan;
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
alternatives considered: in particular any deferred route from the fixed unordered set, any Opus design-review consult
outcome, and every review finding you decided not to follow. The maintainer will ask for these later.

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
last narrowing the original TODO.md entry and adding the mandatory `Plan follow-up` architecture-review TODO entry.
Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
