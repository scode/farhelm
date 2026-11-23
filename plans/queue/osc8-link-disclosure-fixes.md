# Fix false terminal hyperlink mismatch warnings and ignore concealed hyperlink cells

Written against main at 292b9130 on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Fix false terminal hyperlink mismatch warnings and ignore concealed hyperlink cells.** Hovering a program's OSC 8
> hyperlink can falsely warn "Link text does not match where it goes" when the label puts a URL after a non-address
> prefix (such as `ticket-42(https://example.test/tickets/42)`, `ticket-42(example.test/tickets/42)`, or a wrapped
> `label (url)` joined across rows without a space), because the address detector accepts non-scheme punctuation like
> `(` before `://` and non-host punctuation before a scheme-less `host/path`. Separately, concealed (`SGR 8` / `HIDDEN`)
> cells inside an OSC 8 hyperlink are still read into the mismatch check and included in clickable/underlined ranges
> even though their glyphs are not drawn, letting invisible text trigger false warnings or hide a real destination
> mismatch. Tighten both URL-shape checks (keeping Unicode-letter homoglyph and userinfo detection intact) and exclude
> concealed cells from OSC 8 label inspection, relabel detection, and interactive/underlined ranges while preserving OSC
> 8 cell ownership over prose URL and file-path detection underneath.

In product terms: fix two separate bugs in the desktop terminal's hyperlink destination-mismatch check. First, tighten
the check that decides whether a program hyperlink's visible text looks like a web address so non-address text glued to
a parenthesized or bracketed URL (such as `ticket-42(https://example.test/tickets/42)` or
`ticket-42(example.test/tickets/42)`) no longer triggers a false "Link text does not match where it goes" warning, while
keeping homoglyph schemes, lookalike punctuation, and userinfo spoofing warnings working. Second, ignore concealed
(`SGR 8` / `HIDDEN`) terminal cells inside program hyperlinks — excluding them from label inspection, relabel detection,
and clickable/underlined ranges while still letting the hyperlink own those cells against plain printed-URL and
file-path detection underneath — so invisible cells can neither cause false warnings nor mask a real destination
mismatch.

### Where it stands (from planning research at main 292b9130; line numbers approximate)

- **URL-shape detection (`crates/gui/farhelm-grid-model/src/link_disclosure.rs:15-25, 48-61, 101-121, 295-332`):**
  - `ADDRESS` (`link_disclosure.rs:15-19`) is currently `(?i)^(?:[^%s/:]+://|www\.|[^%s/]+\.\p{L}{2,}(?::[0-9]+)?/)`.
    Its first branch matches any non-space, non-`/`, non-`:` prefix before `://`, so
    `ticket-42(https://example.test/tickets/42)` matches `ticket-42(https` as a URL scheme, fails `Url::parse` in
    `mismatch()`, and triggers the loud mismatch warning. Its third branch (`[^%s/]+`) has the same looseness for
    `ticket-42(example.test/tickets/42)`.
  - `mismatch()` (`link_disclosure.rs:109-114`) computes `absolute` with the same loose `!is_js_space && !'/' && !':'`
    check before `://`, which also misclassifies scheme-less inputs with `://` in a query string such as
    `www.example.com?next=https://other`.
  - Keep `paths.rs::SCHEME` (`crates/gui/farhelm-grid-model/src/paths.rs:51`) separate: `paths.rs` must reject ASCII
    single-colon schemes (`javascript:`, `mailto:`), while `link_disclosure.rs` must not treat `javascript:alert(1)` as
    claiming a web address and must still catch Unicode-letter homoglyph schemes like Cyrillic `httрs://`.
  - Tighten `ADDRESS`:
    - Scheme branch: `\p{L}[\p{L}\p{N}+.-]*://` (RFC 3986 scheme syntax generalized to Unicode letters/numbers so
      homoglyph schemes like `httрs://` still warn, while non-scheme punctuation such as `(`, `[`, `<` does not).
    - Scheme-less `host/path` branch: optional userinfo `(?:[\p{L}\p{N}._%+-]+(?::[\p{L}\p{N}._%+-]*)?@)?` followed by
      dotted host labels `[\p{L}\p{N}_-]+(?:\.[\p{L}\p{N}_-]+)*\.\p{L}{2,}(?::[0-9]+)?/` (preserves `github.com/login`,
      `github.com@evil.example/x`, IDN/homoglyph domains, and lookalike dots normalized by `claims_address`, while
      rejecting parenthesized/bracketed prefixes like `ticket-42(example.test/tickets/42)`).
    - Drop the unused `.replace("%s", JS_SPACE)` from `ADDRESS` once `%s` is no longer present (`LEADING_BLANKS` still
      uses `JS_SPACE`).
    - Tighten `absolute` in `mismatch()` to check for a valid scheme prefix before `://` (either ASCII RFC 3986
      `is_ascii_alphabetic` + `is_ascii_alphanumeric / + / - / .`, since `Url::parse` only accepts ASCII schemes and a
      non-ASCII scheme still fails `Url::parse` either way, or the matching Unicode-letter prefix check).
  - `pinned_disclosure_policy_oracle` (`link_disclosure.rs:296-307`) checks against the frozen old-web-client fixture
    `tests/fixtures/links-oracle.json` (`24bc0580`). Across all labels in `tools/generate-links.ts` /
    `links-oracle.json`, `<U+202E>https://example.com/x` (and its whitespace-padded twin, 50 policy pairs across 25
    targets) is the only case whose verdict changes (from `true` to `false`, because `<`, `+`, `>` are no longer
    accepted as scheme characters). Leave the SHA-256-pinned fixture untouched and explicitly assert `mismatch == false`
    for that label in `pinned_disclosure_policy_oracle` with a comment explaining the intentional divergence from the
    old client.
- **Concealed (`SGR 8` / `HIDDEN`) cells in OSC 8 hyperlinks (`crates/gui/farhelm-grid-model/src/links.rs:159-363`,
  `src/paths.rs:320-348`, `src/link_disclosure.rs:176-284`, `SPEC.md:3592-3594`):**
  - `Identities` (`links.rs:159-253`) is the crate's single shared resolver of interactive/visible OSC 8 runs per row
    (its only callers are `GridModel::row_links` and `GridModel::link_label`), and it already filters
    `wide_wrap_padding` runs once (`run.col >= text_end`).
  - Filter `style_flags::HIDDEN` once inside `Identities` (e.g. a small private helper `visible_link(run)` returning
    `run.link.as_ref().filter(|_| run.style.flags & style_flags::HIDDEN == 0)`, used in `previous_piece` and `row` right
    alongside `run.col >= text_end`). That single change handles every consumer consistently:
    - `row_links` (`links.rs:268-318`) skips `HIDDEN` runs automatically, excluding concealed cells from `ranges`
      (hoverable/clickable/underlined span) and `label`, and causing a `HIDDEN` toggle in place to change `TerminalLink`
      so hover re-evaluates.
    - `row_links`'s `evidence` walk (`links.rs:285-306`) stops at concealed-only rows while still keeping
      `first.saturating_sub(1)..=last.saturating_add(1)` in `evidence`, so unhiding an adjacent row stays covered.
    - Anonymous generated-id continuation across soft wraps (`Identities::previous_piece`) stops at concealed cells
      instead of bridging a hidden wrap seam into a quiet match.
    - `link_disclosure.rs`'s `link_label` requires no code changes in PR 2: concealed-only adjacent rows yield
      `count == 0`, visible text with a concealed suffix reads only the visible stretch, and a concealed gap between
      visible stretches of the same explicit/shared `id` yields `count == 2` (conservative fragment check).
  - Plain printed-URL detection (`links.rs:322, 356-363`) already passes `blank_concealed = true` to `word_cells` and
    checks raw `run.link.is_some()` on `row.runs`, so concealed OSC 8 cells already block printed URLs underneath with
    zero changes.
  - Printed file-path lookup (`paths.rs:320-327`, `path_link_at`) currently checks only
    `self.link_at(position).is_some()`, which will return `None` on concealed OSC 8 cells once `row_links` skips them.
    Do **not** switch `paths.rs::read_cells` (`paths.rs:338`) to `blank_concealed = true` (that would alter non-OSC 8
    printed file-path behavior outside this plan's scope). Instead, add a crate-private
    `pub(crate) fn has_osc_link_at(&self, position: Position) -> bool` on `GridModel` routed through
    `self.selection_line(position.line)` and `position.col < row.text_columns(cols)` (or share a 3-line
    `pub(crate) fn has_osc_link(&self, from: u16, to: u16) -> bool` on `GridRow` with `links.rs:356-360`), and have
    `path_link_at` return `None` when either `self.link_at(position).is_some()` or `self.has_osc_link_at(position)`.
  - `crates/gui/farhelm-gui` needs no changes: `TerminalView::link_at`, `underlined_rows`, `sync`,
    `AttachedTerminal::click_visibility`, and `FileLinks::ready_for` already delegate to `GridModel` and `TerminalLink`
    equality (`ranges`, `label`, `evidence`), and already have GPUI tests. Keep all new tests in `farhelm-grid-model`.
  - Update `SPEC.md:3592-3594` beside the existing concealed printed-URL rule to document that concealed (`SGR 8`) cells
    are also excluded from a program hyperlink's hoverable/underlined span and visible label comparison while the
    hyperlink keeps cell ownership over prose detection (background-colored text remains out of scope).

### Acceptance criteria

1. `link_disclosure` does not warn on non-address prefixes before `://` or scheme-less `host/path` — including
   single-row and multi-row wrapped labels such as `ticket-42(https://example.test/tickets/42)`,
   `ticket-42(example.test/tickets/42)`, and `[label](https://example.test/x)` — while still warning on Unicode-letter
   homoglyph schemes (`httрs://...`), lookalike punctuation (`httpsː//...`, `github。com/...`), leading invisible/blank
   characters (`\u{202E}https://...`), and userinfo spoofing (`https://good.example@evil.test/`,
   `good.example@evil.test/x`), and without misclassifying `www.example.com?next=https://other` as an absolute URL.
2. `pinned_disclosure_policy_oracle` remains green against the untouched `tests/fixtures/links-oracle.json`, explicitly
   asserting `mismatch == false` for the one intentional divergence (`<U+202E>https://example.com/x` and its
   whitespace-padded variant).
3. Concealed (`SGR 8` / `HIDDEN`) cells inside an OSC 8 hyperlink are excluded from:
   - the hyperlink's hoverable/clickable/underlined ranges (`row_links` / `link_at`),
   - its visible label comparison (`link_disclosure`) and relabel identity (`TerminalLink::label`), so concealed text on
     the same row or an adjacent row can neither trigger a false mismatch warning nor hide a real mismatch (e.g.,
     visible `https://good.example` + concealed `.evil.example` targeting `https://good.example.evil.example` warns;
     visible `ticket-42` + concealed `(https://example.test/tickets/42)` does not warn), and
   - anonymous generated-id continuation across a concealed wrap seam.
4. Toggling `HIDDEN` on cells of an OSC 8 hyperlink changes the returned `TerminalLink` (`ranges`, `label`, or
   `evidence`) so an existing hover re-evaluates.
5. Concealed OSC 8 cells still own their cells against plain printed-URL detection (`row_links`) and printed file-path
   detection (`path_link_at`), without altering non-OSC 8 printed file-path behavior (`paths.rs::read_cells` keeps
   `blank_concealed = false`).
6. `SPEC.md` is updated beside the existing conceal rule; focused tests in `crates/gui/farhelm-grid-model` cover all new
   behavior and regressions using only generic hypothetical examples; and the `TODO.md` entry is removed in the last PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-23):**

- Schedule both fixes as one combined plan with a matching `TODO.md` entry in `## Near term`.
- Privacy constraint: use only generic hypothetical examples in `TODO.md`, the plan, tests, and commits (no real
  incident details, hostnames, or issue-tracker ID formats from the original report).
- URL-shape tightening scope: tighten both the `://` scheme rule and the scheme-less `host/path` rule.
- Concealed (`SGR 8` / `HIDDEN`) cells in OSC 8 hyperlinks: exclude them from label checks, relabel detection, AND
  clickable/underlined ranges, while keeping OSC 8 cell ownership over plain URL and file-path detection underneath.
- PR stack shape: two PRs — (1) URL-shape tightening + tests, (2) concealed OSC 8 cells + `SPEC.md` + `TODO.md` removal.
- Per-PR review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort with the general charter.
- Release note (highlight, verbatim):
  `Fixed some false positive triggers of a safety check that alerts you when a clickable visible URL/link actually points to a different link than what you see on screen.`

**Binding repository constraints:**

- `SPEC.md:3571-3595` (terminal links, OSC 8 mismatch warning, and concealed cells).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals (reviewed by fresh-context planning scope reviewer):**

- Filter `style_flags::HIDDEN` once inside `Identities` (`crates/gui/farhelm-grid-model/src/links.rs`) rather than
  duplicating `HIDDEN` checks across `row_links` and `link_disclosure::link_label`. That keeps `evidence`, `ranges`,
  `label`, generated-id wrap continuation, and `link_label` aligned in one place and avoids touching
  `link_disclosure.rs` in PR 2.
- Keep `has_osc_link_at` crate-private (`pub(crate)`), routed through `selection_line` and `text_columns` (close call:
  sharing a 3-line `GridRow::has_osc_link(from, to)` helper with `links.rs:356-360` vs. keeping a 6-line
  `has_osc_link_at` in `links.rs` is left to the executing agent and logged as a `DECISION`).
- Keep `paths.rs::SCHEME` separate from `link_disclosure.rs::ADDRESS`, drop unused `.replace("%s", JS_SPACE)` from
  `ADDRESS`, and leave `tests/fixtures/links-oracle.json` untouched while asserting the one intentional divergence
  (`<U+202E>https://example.com/x`) explicitly in `pinned_disclosure_policy_oracle`.
- Keep all new tests in `crates/gui/farhelm-grid-model` without adding redundant GPUI-level tests since `farhelm-gui` is
  unchanged.

## Outline

1. `plan/osc8-link-disclosure-fixes/01-tighten-osc8-address-detection`: tighten `ADDRESS` (both the `://` scheme branch
   and the scheme-less `host/path` branch) and `absolute` in `crates/gui/farhelm-grid-model/src/link_disclosure.rs`,
   drop the unused `JS_SPACE` replacement from `ADDRESS`, explicitly assert `mismatch == false` for the one intentional
   divergence (`<U+202E>https://example.com/x`) in `pinned_disclosure_policy_oracle` without modifying
   `tests/fixtures/links-oracle.json`, and add unit tests in `crates/gui/farhelm-grid-model` covering hypothetical
   non-address prefixes (single-row and multi-row wrapped), scheme-less `host/path` false positives, homoglyph
   schemes/hosts, lookalike punctuation, userinfo spoofing, and query-string `://`
   (`www.example.com?next=https://other`). `fix: tighten terminal hyperlink address detection` (with `kind: fixed`
   changelog fragment).
2. `plan/osc8-link-disclosure-fixes/02-exclude-concealed-osc8-cells`: filter `style_flags::HIDDEN` once in `Identities`
   (`crates/gui/farhelm-grid-model/src/links.rs`) so concealed OSC 8 cells are excluded from interactive/underlined
   ranges, `TerminalLink::label`, `evidence`, generated-id wrap continuation, and `link_disclosure`; add crate-private
   `has_osc_link_at` (respecting `selection_line` and `text_columns`) and call it in `GridModel::path_link_at`
   (`crates/gui/farhelm-grid-model/src/paths.rs`) so concealed OSC 8 cells keep ownership over printed file paths
   without changing `paths.rs::read_cells`; update `SPEC.md:3592-3594`; add grid-model tests in `tests/links.rs` and
   `tests/paths.rs` covering concealed suffixes, concealed-only adjacent rows, concealed gaps between visible runs,
   concealed wrap seams, toggling `HIDDEN`, and concealed OSC 8 cells blocking plain URLs and printed paths; amend PR
   1's changelog fragment; and remove the `TODO.md` entry ("Fix false terminal hyperlink mismatch warnings and ignore
   concealed hyperlink cells"). `fix: ignore concealed cells in terminal hyperlinks` (amends PR 1's changelog fragment).

## Fallbacks

- None expected; escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-osc8-link-disclosure-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-osc8-link-disclosure-fixes-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run `cargo nextest run -p farhelm-grid-model`, `cargo fmt --all -- --check`,
`cargo clippy -p farhelm-grid-model --all-targets -- -D warnings` (and `cargo clippy --all-targets -- -D warnings` at
the tip if appropriate), `dprint check` on changed Markdown (`SPEC.md`, `TODO.md`),
`python3 releasing/check-changelog.py format` on the changelog fragment, and `python -B scripts/check-test-sleeps.py` as
`docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to every test you add or change. Never take a
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
- The last PR of the stack removes the TODO.md entry this plan covers ("Fix false terminal hyperlink mismatch warnings
  and ignore concealed hyperlink cells"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

Fixed some false positive triggers of a safety check that alerts you when a clickable visible URL/link actually points
to a different link than what you see on screen.
