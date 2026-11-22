# Attach files pasted from a Wayland file manager

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Attach files pasted from a Wayland file manager.** A terminal paste can ignore copied files or type their displayed
> paths. In the vendored Wayland `clipboard.rs`, read `text/uri-list` before image or text, as SPEC.md's “Attachments”
> requires. Accept only local `file:` URLs. Keep ordinary paste working when the list has only web links or invalid
> entries by reading the next available flavor from the captured offer. Cancel the paste if that offer has expired. Keep
> the vendor patch and its tree in sync, and test both a file offer and a browser link offered as URI and plain text.

In product terms: when a user copies files in a Linux/Wayland file manager and pastes into a Farhelm session terminal,
upload and attach those local files (in order, through the existing attachment pipeline) rather than ignoring them or
typing their local paths. When a clipboard's `text/uri-list` holds only web links (such as a browser copy) or invalid
entries, fall back cleanly to reading the next available flavor (image, then plain text) from the **same captured
clipboard offer**, cancelling the paste if that offer expired mid-read.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Vendored Wayland clipboard (`vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs`):**
  - `FILE_LIST_MIME_TYPE = "text/uri-list"` is already defined (`clipboard.rs:23`) and `url = "2.2"` / `smallvec` are
    already in `vendor/gpui-pre-linux/Cargo.toml` (used by Wayland DnD at `client.rs:2984-2994`).
  - Leave `vendor/gpui-pre-linux/src/linux/wayland/client.rs` completely untouched: every `Selection` already routes
    through `Clipboard::set_offer`, and `wayland_client::Proxy::is_alive(&offer.inner)` detects proxy destruction. Keep
    a small, silent RFC 2483 `parse_file_uri_list` helper local to `clipboard.rs` (avoiding `client.rs` DnD's
    `log::error!` on web links and avoiding patch collisions with `wayland-drop-position`).
  - `DataOffer::payload_mime_type()` (`clipboard.rs:137-153`) currently selects only `ImageFormat::iter()` then
    `TEXT_MIME_TYPES`, skipping `text/uri-list`.
  - `Clipboard::read_async(&self, executor: &BackgroundExecutor)` (`clipboard.rs:292-339`) reads a single flavor from
    `current_offer`. Update it to attempt `text/uri-list` first when offered, parse RFC 2483 lines (ignoring blank lines
    and `#` comments, accepting `\r\n` and `\n`, parsing each URI via `url::Url::parse` + `url.to_file_path()` so only
    local `file:` URLs are accepted and percent-decoded).
  - **Mixed lists:** Per the maintainer's decision, attach valid local `file:` paths and ignore any non-local/invalid
    lines in the same list (matching Wayland DnD). When zero valid local files are parsed (e.g. only `https://` web
    links or invalid/non-UTF-8 lines), fall back to reading image or plain text from the **same captured offer** — never
    a replacement offer.
  - **Offer expiry:** Track offer liveness inside `clipboard.rs` via an `Arc<AtomicU64>` generation counter (or
    per-offer `Arc<AtomicBool>`) bumped/invalidated in `Clipboard::set_offer`, combined with `offer.inner.is_alive()`.
    If the captured offer expired before or during the `text/uri-list` read, cancel the paste (`Ok(None)` / silent
    error) rather than reading a newer offer. A pipe timeout or I/O error on `text/uri-list` also cancels the paste
    rather than waiting another timeout on a dead producer.
  - Keep sync `Clipboard::read()` (`clipboard.rs:269-285`, used by text inputs) text-first and completely untouched.
- **Vendor patch & doc sync (`vendor/README.md`):**
  - Update `vendor/patches/gpui/attachment-clipboard/gpui-pre-linux-0.3.12.patch` in sync with `vendor/gpui-pre-linux/`
    so the full `gpui-pre-linux` patch series applies cleanly from the pristine `.crate` (`vendor/README.md`, lines
    1319, 1329, 1360-1362, 1443-1453). Watch out for
    `vendor/patches/gpui/modified-notices/gpui-pre-linux-0.3.12.patch:37-46`, which anchors on lines 1-6 of
    `clipboard.rs`: keep those top import lines intact (or refresh `modified-notices` too) and update the numstat
    table/totals in `vendor/README.md`.
  - Update stale comments noting Wayland lacks file lists in
    `crates/gui/farhelm-gui/src/terminal/attached.rs:1615-1617`,
    `crates/gui/farhelm-gui/src/terminal/attached/tests.rs:8200-8201`,
    `crates/gui/farhelm-gui/src/terminal/attachments.rs:35-37`, and `vendor/README.md`.
- **GUI attachment pipeline:** `attached.rs:1629-1668` and `attachments.rs:157-176` already classify and upload
  multi-file `ClipboardEntry::ExternalPaths` in order (and refuse directories visibly).
- **Tests:** Extend `mod attachment_clipboard_tests` (`clipboard.rs:44-93`) via its `ReceiveData` / pipe/offer seam to
  unit-test: (1) local `file:` URI list (multi-file, spaces/percent-encoding, `#` comments, mixed local + web link
  taking valid local files), (2) browser link offered as `text/uri-list` (`https://...`) + `text/plain` falling back to
  plain text (and image fallback when `image/png` + web URI), and (3) expired/replaced offer cancelling paste cleanly.

### Acceptance criteria

1. Wayland `Clipboard::read_async` checks `text/uri-list` before image or plain text, returning
   `ClipboardEntry::ExternalPaths` for valid local `file:` URLs (percent-decoded, ignoring comments/blank lines and
   non-local/invalid lines in mixed lists). Sync `Clipboard::read` stays text-first and untouched.
2. When `text/uri-list` yields no local `file:` paths (e.g. a browser link offered as `text/uri-list` + `text/plain`),
   `read_async` falls back to image then plain text from the **same captured offer**.
3. If the captured offer has expired or been superseded before fallback (or the pipe errors/times out), the paste is
   cancelled cleanly rather than reading a newer offer.
4. `vendor/patches/gpui/attachment-clipboard/gpui-pre-linux-0.3.12.patch`, `vendor/gpui-pre-linux/`, `vendor/README.md`,
   and stale GUI comments (including `attached/tests.rs`) are all kept in sync, and the full patch series applies
   cleanly per `vendor/README.md`.
5. Unit tests cover local file URI lists, mixed lists, browser-link URI + plain text fallback, image fallback, and
   expired-offer cancellation. The TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- For a mixed `text/uri-list` containing local `file:` URLs alongside non-local/invalid lines, attach the valid local
  files and ignore the non-local/invalid lines (matching Wayland drag-and-drop).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `vendor/README.md`: never edit `vendor/` trees without keeping their patch files in sync and verifying clean patch
  application.
- `.agents/validation.md:629-635`: run the vendored `gpui-pre` test suite whenever anything under `vendor/gpui-pre*` or
  `vendor/patches/gpui/` changes.
- `SPEC.md`, "Attachments" (lines 3608-3645).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Update `vendor/patches/gpui/attachment-clipboard/gpui-pre-linux-0.3.12.patch` in place (logical owner of `read_async`)
  while keeping `clipboard.rs`'s top-of-file imports intact for `modified-notices`, and keep `client.rs` untouched.
- Deliver as 1 cohesive PR (`fix: attach files pasted from a Wayland file manager`).

## Outline

1. `plan/wayland-clipboard-file-paste/01-wayland-clipboard-file-paste`: implement `text/uri-list` local-file parsing,
   same-offer image/text fallback, and expired-offer cancellation entirely inside
   `vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs` (keeping `client.rs` untouched), add unit tests in
   `attachment_clipboard_tests`, regenerate `vendor/patches/gpui/attachment-clipboard/gpui-pre-linux-0.3.12.patch` and
   verify full patch-series sync per `vendor/README.md`, update stale comments in `attached.rs`, `attached/tests.rs`,
   `attachments.rs`, and `vendor/README.md`, add a `kind: fixed` changelog fragment, and remove the TODO.md entry
   ("Attach files pasted from a Wayland file manager"). `fix: attach files pasted from a Wayland file manager`.

## Fallbacks

- Running standalone `vendor/gpui-pre-linux` unit tests requires `--no-default-features --features wayland` (because
  `x11` carries uncompiled hunks per `vendor/README.md:1410-1412`) plus `--config` path overrides for `gpui-pre` and
  `gpui-pre-wgpu`. Record the exact command in the working log.
- If adding a new patch layer at the end of `vendor/patches/gpui/` avoids fragile hunk shifts against `modified-notices`
  more cleanly than updating `attachment-clipboard` in place, that is an acceptable alternative; log it as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-wayland-clipboard-file-paste-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-wayland-clipboard-file-paste-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run `gpui-pre-linux` `attachment_clipboard_tests` (`--no-default-features --features wayland` with path
overrides), the required vendored `gpui-pre` test suite (`.agents/validation.md:629-635`:
`cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record --test hitbox_occlusion`),
`farhelm-gui` `terminal::attachments` tests, full vendor patch verification per `vendor/README.md`,
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Attach files pasted from a Wayland file
  manager"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

No release note needed.
