# Farhelm M6.5: test-suite backfill

NOTE: This is the plan for milestone 6.5 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Pay down the test debt PLAN.md's ladder parked here — the items that stayed small, stable, and low-risk while the focus
was end-to-end progress — and fix the one product bug parked alongside them. This is deliberately a narrow milestone:
the ladder entry is mostly a ledger, and most of the ledger stays exactly where it is. What ships is the four tractable
items: the JS unit-harness decision plus `onBinary` coverage, a mouse-mode fake-agent script with the reattach e2e it
enables, the reusable drive-a-real-agent Playwright helper, and the titlebar-overflow fix.

The seam-requiring parked items do NOT ship (user decision 2026-08-06): the deterministic attach-races-rename
interleaving, the takeover-before-marker forcing, the rename-commit cancellation test, the claim-released-before-
reply-probe test, the supervisor-side mid-replay proof, the outcome-transition failure forcing, the deterministic
ack-enqueued observation, and the scope-probe timeout-versus-retry component test all need seams the production code
deliberately does not have, and building seams for test-only value is a trade this milestone declines. They stay in the
ladder's ledger, unchanged. The flake ledger itself also stays: the discriminators stand, and the two CI-quieting
candidates (a concurrency group per ref, lowering e2e `SLOTS` under CI) remain undecided pending evidence, per the
ladder's own reasoning.

## User-visible outcome

One fix: a session whose invocation runs long — 120-odd characters is enough — no longer overflows the session view's
titlebar in a way that pushes the rename control out of reach. Everything else in this milestone is test infrastructure;
nothing else a user sees changes.

## Scope

### In

1. **JS unit harness: node's built-in test runner.** The repo's only JS testing today is Playwright; `onBinary`'s byte
   conversion needs a unit seam, and the M1 review judged a JS unit runner premature for one small function. The
   decision now: `node --test`, no new dependencies at all — node is already a CI requirement via Playwright, and the
   built-in runner covers "a handful of pure functions" indefinitely. vitest and jest were considered and rejected: each
   adds a dependency tree and a config surface to test what is initially one function, and nothing about the asset-JS
   layer (plain scripts, no bundler) benefits from their module tooling. Revisit only if the JS unit surface grows past
   a handful of files.

   The seam: the conversion inside `term.onBinary` (`terminal.js` — binary string to `Uint8Array`, byte-for-byte) moves
   into a small shared plain-script asset loaded before `terminal.js`, following the exact loading pattern xterm.js and
   addon-fit already use (no bundler, self-contained). The helper exposes itself as a browser global and additionally
   via `module.exports` when that exists, so the page and `node --test` load the identical file rather than a copy. Unit
   tests live outside `assets/` (test files must not ride into the served bundle), under `crates/farhelm-ui/js-tests/`,
   wired into CI as a cheap step alongside the existing jobs. Coverage pins the byte-for-byte contract at the
   byte-domain boundaries — 0x00, 0x7f, 0x80, 0xff — plus empty input and a mouse-report-shaped sequence; the contract
   is "every codepoint becomes its low byte", not any particular operator the implementation happens to use for it.
   SPEC_impl.md's Testing section gains the new harness in the same PR — that section enumerates the testing story, and
   a layer it does not name is a layer the next reader will not run.

2. **Mouse-mode fake-agent script plus the reattach-restoration e2e.** A new `farhelm internal fake-agent` script that
   enables mouse reporting on cue — separate cues for legacy button tracking alone and for SGR encoding on top, because
   the two exercise DIFFERENT client paths: xterm.js emits SGR reports (pure ASCII) through `onData`, while
   legacy-encoding reports carry high bytes and go through `onBinary`, which is exactly the seam item 1 extracts. The
   legacy leg of this test is therefore what proves the extracted helper's browser wiring end to end; without it the
   unit test could pass while the browser-side global sat unwired. The script echoes every byte it receives in a
   hex-visible form, so a test can assert that a mouse click actually reached the agent as a mouse sequence — and the
   fake agent already owns hex-visible byte echoing (the binary script's reader); the mouse script reuses that machinery
   as a shared body or prelude rather than growing a second byte-echo implementation. The e2e test pins the one
   reattach-restoration branch nothing exercises end to end: the Rust e2e suite already proves PaneModes re-synthesis
   for bracketed paste, but mouse-mode restoration — and the full browser path, DOM click through xterm.js encoding to
   the agent — has no coverage at all. Enable legacy mouse mode, click and assert the report arrives (onBinary proven),
   detach and reattach, click and assert again (restoration proven), then enable SGR and assert that path too. Named
   test: `mouse-modes-restored-on-reattach`. SPEC_impl.md's Testing section names the fake agent's script list
   (`basic|altscreen|binary`), so the new script updates that list in the same PR, per that document's standing sync
   rule.

3. **The drive-a-real-agent Playwright helper.** A reusable helper (under `e2e/tests/`) that bakes in the lessons the
   first agent-driven smoke test learned the hard way: Claude Code's trust dialog must be detected and accepted before
   anything else; its fast-typing paste heuristic swallows an Enter that arrives glued to the prompt text, so submission
   types the prompt and sends Enter separately after a settle; and reply detection waits on the agent's reply markers
   rather than on silence. Honesty about CI: real agents cannot run there (no vendor auth), so the helper's real-agent
   spec is env-gated (`FARHELM_REAL_AGENT=1`) and skips loudly otherwise, exactly like the cgroup tests skip without a
   user manager — while the helper's mechanics that do not need a real agent (submission pacing, marker waiting) get a
   CI-run exercise against the fake agent. Real-agent smoke remains manual per SPEC_impl.md's testing section; this item
   makes the next manual round cheap instead of re-learned.

4. **The titlebar-overflow fix.** The session view's header carries the session's metadata — the working directory and
   the agent invocation — beside the title and the rename affordance, and a long INVOCATION is what overflows it:
   120-odd characters push the rename control out of reach. (An auto-generated title cannot reproduce this — titles
   default from the cwd basename, not the invocation.) A real UI failure on a surface every session has, found through
   e2e tooling whose snapshot paths embedded exactly such an invocation, rather than by a test. The fix is layout: the
   header's overflowable text truncates (ellipsis) under pressure — the long invocation first and foremost, and a
   user-supplied long title by the same mechanism — while the rename control keeps its place and its clickability. Named
   test: `long-invocation-titlebar-rename` — create a session with an ordinary title and an invocation well past 120
   characters, assert the rename control is visible and completes a rename.

   **Amended while building this item (2026-08-06): the bug's closed-state form no longer reproduces, and its open-state
   sibling turned out to be the real survivor.** The review swarm proved — and a pre-change rerun confirmed — that the
   CLOSED header already behaves: the metadata span truncates (its `overflow: hidden` zeroes the flex minimum on its
   own), and the rename control sits BEFORE the metadata in flex order, where no amount of overflow can displace it —
   the M5-era header rebuild fixed the M4-era symptom incidentally, and nobody closed the ledger entry. But the
   strengthened test the review demanded then failed for real: with the rename form OPEN, the shared `.rename-form`
   rule's zero flex-basis gives the form zero shrink-weight, and a long invocation squeezes its input to an unusable
   sliver. That open-state squeeze is what this item's fix actually ships (a scoped content-based basis for the
   titlebar's form, with the metadata as what yields), verified fail-before/pass-after.
   `long-invocation-titlebar-rename` pins the reachability contract in BOTH states — clipped metadata, fully visible
   control, a rename completed through the visible save control — against future header rework.

### Out (deliberately)

Everything listed in the Goal's second paragraph: seam-requiring test items, flake-ledger changes, CI-quieting
candidates. Also out: any refactor work — the architecture-assessment refactors running alongside this milestone are
their own `refactor:` PRs, deliberately outside this plan (same discipline as M4.5: functional no-ops never interleave
with functional changes). And the JS harness deliberately does NOT grow beyond `onBinary`'s seam — no speculative
porting of other terminal.js logic into testable helpers; each future extraction pays its way when something needs it.

## Testing decisions (settled while planning)

The new e2e tests land in per-area spec files, not in `terminal.spec.ts` — that file is 13,792 lines and every remaining
milestone adds named tests, so new specs start their own files (`mouse-modes.spec.ts`, `real-agent.spec.ts`,
`titlebar.spec.ts`) and `terminal.spec.ts` stops growing. Whether the existing file gets split is the architecture
assessment's question, not this plan's; the rule here is only that M6.5 adds no new mass to it.

The `node --test` step runs in the existing test job rather than a new CI job — it is milliseconds of work and a new
job's scheduling overhead would dwarf it.

The mouse e2e drives real xterm.js mouse encoding by dispatching real pointer events in the page, not synthetic escape
sequences down the socket — the point is the full path: DOM click → xterm.js encoding → WebSocket → supervisor → tmux →
agent.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies (M6 moves to history).
2. The JS unit harness and `onBinary` coverage (the helper extraction, the `js-tests/` layout, the CI step).
3. The mouse-mode fake-agent script and `mouse-modes-restored-on-reattach`.
4. The drive-a-real-agent helper, its fake-agent-backed CI exercise, and the env-gated real-agent spec.
5. The titlebar fix and `long-invocation-titlebar-rename`.

## Acceptance

M6.5 is done when all of the following hold, pinned by automated tests:

1. `node --test` runs in CI and covers the extracted byte-conversion helper, which is the same file the page loads. The
   extraction itself is checked immediately by the e2e suite still passing; the browser WIRING of the extracted helper
   is proven by the mouse test's legacy-encoding leg (item 2), the one path that actually drives `onBinary` — the suite
   passing alone would not catch an unwired browser global.
2. `mouse-modes-restored-on-reattach` passes: mouse modes enabled by the agent survive a detach/reattach cycle, proven
   by a post-reattach click arriving at the agent as a mouse report.
3. The real-agent helper exists, its CI-runnable mechanics are exercised against the fake agent, and the env-gated
   real-agent spec skips loudly in CI (visible skip reason, same discipline as the cgroup tests).
4. `long-invocation-titlebar-rename` passes: a session with an over-long title still renames through the UI.
5. The full CI gate is green.

## Risks retired by this milestone

- The `onBinary` conversion — the one spot where input bytes are reconstructed by hand — gets pinned before M6.75's
  status work adds more traffic-dependent behavior around the terminal path.
- Mouse-mode restoration — the one PaneModes branch with no end-to-end coverage — now fails a named test on regression
  instead of a future manual session, and the browser half of the path (DOM click to xterm.js encoding) is exercised at
  all for the first time.
- The next real-agent manual round starts from a working helper instead of re-discovering the trust dialog and the paste
  heuristic — which also lowers the cost of every future "does it work with the real thing" question.
- The titlebar bug stops biting daily use and stops silently constraining how e2e tooling may lay out the paths that end
  up inside invocations.
