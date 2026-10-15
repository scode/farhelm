## What this was about

A session could switch into a no-checks mode after it started while the sidebar still showed its original permission
shield. The maintainer chose best-effort status-line reading for Claude, Grok, Muse and Cursor: follow definite mode
changes without treating the screen as permission authority.

The shield now turns YOLO when Farhelm recognizes the agent's no-checks footer. Hover and screen-reader text name the
observed mode, explain that detection is best effort, and retain the launch explanation and command. A definite non-YOLO
reading restores the launch shield; an unreadable screen retains the previous observation. The shield never promises
more protection than the launch. Commands declaring one of these agents are covered too.

## Things you should know

Farhelm's host confirmations, approvals and permission choices for spawned sessions still use the saved launch alone.
There is no database migration or protocol-version change. Old peers can omit the new advisory field.

Cursor currently displays Run Everything only while enabled. Turning it off removes the label rather than supplying a
definite non-YOLO label, so its observed YOLO shield stays until the agent or supervisor restarts. A supervisor restart
clears all observed modes until new definite samples arrive. Missed or late detection and vendor wording changes remain
within the agreed best-effort limit; allow rules, hooks and managed policy are not visible.

Capture used Claude 2.1.300, Grok 1.0.52, Muse 1.4.7 and Cursor 2026.10.14-0f9edde without sending model prompts,
signing in, upgrading agents or editing their settings. The agents may retain workspace-trust records for the disposable
directories named claude-mode, cursor-mode and muse-mode. Exact private locations are recorded in
farhelm-plan-detect-mid-session-yolo-log.md beside the checkouts. Muse's YOLO capture used a run-only flag in a separate
muse-yolo-mode directory; its menu's default-saving choices were left unchanged. All capture processes were stopped.

## Open questions and possible follow-ups

No maintainer decision is needed. Future agent versions may need updated footer readers and captures. Cursor's lack of
an explicit off label is documented rather than guessed around.

## The PRs

- [#2063](https://github.com/scode/farhelm/pull/2063/changes): observed permission modes in session shields, including
  compatibility tests, specifications, agent documentation and the changelog fragment.

## Checks run, reused and skipped

- Passed 218 selected Rust tests across the protocol, supervisor, helm and UI: wire compatibility, real captured
  footers, status replies, cached lists, HTTP contracts, row rendering and rename preservation. Recorder run
  `b908c156-463f-4e3f-acd7-d3ee46da766b`; zero retries, four slots, pinned tmux. Its retained output has no runtime
  `SKIPPED` markers. Tests outside that selection were deliberately excluded.
- An earlier five-test feature selection passed in run `0449df17-a925-4dfd-995f-49c6176bcab4`. The initial run
  `08f89156-a0ce-463f-99dd-683f334eab57` failed compilation because the new UI test omitted an existing launch field; it
  was corrected before the successful runs. There were no runtime test failures.
- Passed `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, covering both
  test-enabled and shipped supervisor configurations. Also passed `cargo fmt --all -- --check`, the isolated
  `python -B scripts/check-test-sleeps.py` check (299 delays, none unexplained), changed-file `dprint check`, and
  `python3 releasing/check-changelog.py format`.
- Passed `bun install --frozen-lockfile` and `bun run build` for the docs: 32 pages and valid internal links. Reused
  that build after a plain-wording clarification about supervisor restarts; links, frontmatter and page syntax did not
  change.
- Inspected every incoming main change before rebasing: only another plan's queue claim. Validation still covers the
  resulting PR head `3be27a968d79f7edda8577ad4b7db5b5fb5e94b1`; the rebase changed no product code.
- Skipped browser tests: no new browser communication, markup structure, styling or event relay; the badge's computed
  result feeds the existing hover and accessible description, covered by the selected row-render tests. No asset-JS,
  database, installer or release behavior changed, and no executable doc examples were added, so their suites and the
  full workspace runtime battery would add no useful coverage here.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no remaining source issues after corrections. It
also reviewed the rename-preservation and declared-command regressions. The valid independent wording cold read
understood the intended display/permission boundary and found the claims consistent with the change. The first wording
reader's blind phase was contaminated and was discarded. Native review token usage was unavailable; private execution
evidence records that gap.

### Landing

Landed as #2063, the round's only plan. Since its base, main had gained the planning batch that scheduled it, a TODO
entry and changes to the Mac development-build scripts; while it landed, two other plans delivered their reports. None
of these touch the supervisor's screen reading, the session list or the sidebar shield.

#### Review

An independent fresh-context review found nothing blocking. Per-agent reading lives where the harness map says, in
exhaustive matches; no admission, inheritance, approval or YOLO-guard path reads the new field, and a false reading can
only add a YOLO alarm or fall back to the launch shield. The field is additive and omitted when false, which needs no
protocol bump; cached rows keep their last value and a restart or reload resets it. The captured screens contain no
usernames, home paths, hostnames, emails or keys. No existing browser test pins a shield tooltip for an agent this
reads.

#### Fixes made while landing

- Claude's plan mode was read as a definite non-YOLO mode, but the captured screen itself shows Claude saying that a
  session started with bypass permissions keeps bypassing in plan mode. Plan mode now keeps the last reading.
- SPEC.md and the agents page said any definite return to a non-YOLO mode restores the launch shield, but only some of
  those footers were captured and some are not recognized at all. They now say a recognized mode restores it and an
  unrecognized one keeps the YOLO shield.
- A red YOLO shield's hover and screen-reader text began by describing the safer launch mode. The observation now comes
  first, followed by "Launched as:" and the launch description; SPEC_impl.md says the same.
- The agents and Cursor pages used "supervisor" without introducing or linking it, and described a restart by its
  mechanism; both now follow the editorial rules.
- The captured-screen test now reads the same trimmed tail the sampler reads, and puts the mode words on each of the
  lines just above the footer instead of at the top of the screen, where any reader anchored at the bottom would pass.

#### Left as is

- The two Claude captures include what looks like the capturing account's own Claude status line (a working-directory
  name, a model name and a context bar). Nothing personal, but unlike a default Claude screen; worth replacing at the
  next capture.
- Several recognized "asks first" modes, including Claude's auto mode, which is what Guarded launches use, come from
  planning notes rather than captures. If the guess is wrong, the YOLO shield stays after a switch back, which fails
  safe. Capturing an auto-mode footer would settle it.
- Nits: test struct literals set the new field where a base already supplies it; a stray line ending in "Run Everything"
  just above Cursor's footer gives a false YOLO that lasts until restart; an agent running without checks could redraw a
  non-YOLO footer itself, which at worst returns the launch shield.

#### Checks

- Run now, on the rebased change with all fixes in:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (299 delays, none missing a
    reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's clippy
    with the desktop feature and `cargo check -p farhelm-desktop`;
  - the changed reader and shield tests first, run `c798b6ca-ebce-4c82-bca8-74864d7d52e9`: 3 passed;
  - the protocol, helm and supervisor packages in full through the recorder with pinned tmux, run
    `7a9238a4-6ac1-4329-8910-1811cd06f138`: 2365 passed, 2 skipped;
  - the UI crate in full with the desktop feature, run `f8e66bca-649d-4dbd-8bb3-8d3b333cf807`: 636 passed;
  - the JS harness (248 passed) and the website build;
  - the sidebar, OMP composer, approval layout, YOLO guard, guarded, custom and remembered permission, and
    tooltip-coverage specs on Chromium and WebKit, run `8f35815f-1765-4bfa-b6f6-630632ceafe4`: 290 passed.
- Not run: the rest of the Rust workspace, whose changed files only gain the new field in struct literals and compile
  under clippy, and other browser specs.
