# E2E fixture contracts: write tests against these, or lose an afternoon

The map of non-obvious contracts this suite's fixtures impose, learned the hard way during the 2026-08 burndown's test
migrations. The helpers' docstrings in `helpers/fleet.ts` carry the per-function details; this file is the orientation
pass a new spec's author should read first. Inherited from the retired MOPUP_TODO.md (2026-08-16).

Learned during the burndown's test migrations; the helpers' docstrings in `e2e/tests/helpers/fleet.ts` carry the
details, this is the map:

- **Fabricated helm replies need three things** or the client rejects/misreads them: the `x-farhelm-build` header (probe
  the real helm for its stamp — a missing stamp latches skew and the page stops reading), COMPLETE serde enum variants
  (`HostPhase` is `tag="phase"` with required per-variant fields; a bare `{phase: "connected"}` fails decode and renders
  the host list's error line instead of rows), and a `matching` count (a missing `matching` reads as an old helm and
  takes the count banner's "filter was ignored" wording).
- **Auto-select changes every test's page load**: a session attaches at `goto` before any click. Tests that stage route
  holds or stubs around a specific session's first reads must `pinAutoSelect(page, otherId)` BEFORE `goto` (the helper
  fetches the helm identity and writes the keyed record — a bare id is silently ignored). `__farhelmTermReady` is true
  from load, so it no longer gates "the session I just acted on is up" — use title-based completion signals.
- **Only genuinely on-demand surfaces need open helpers**: `openRowMenu` awaits its floating menu, while the permanent
  host selector is ready when its selected option and the particular request or DOM outcome under test are observed.
  `openHostsPanel` awaits the permanent host list and opens its global details disclosure. A bare-DOM
  `querySelector().click()` on a not-yet-mounted control is a silent no-op.
- **"Leaving" a session means selecting another** (no back button): bounce through the shared `e2e-session` row, and
  prove teardown via stashed handles (socket readyState, instance identity) rather than gone-entirely windows — the
  replacement mount owns the globals immediately.
- **`created_at` is second-granular with an id tiebreak**: two sessions created in the same second have arbitrary merged
  order, so "newest" assertions need a >1s gap between creates.
- **Playwright's fake clock ticks between `install()` and `pauseAt()`**: pause at a strictly future instant or a loaded
  box throws "Cannot fast-forward to the past".
- **The bundled terminal font is a mount precondition, so no post-mount font swap exists**: terminal.js waits for both
  JetBrains Mono weights before constructing a terminal (its "## Font settling before mount" header), so the old
  post-mount re-fit/re-flow class of bug is gone. Other resizes (window resize, a user font-size step) still reflow the
  pane, and content printed early (a startup banner, a ready marker) can still end up outside the CURRENT viewport
  window; a viewport-scoped poll for such early content is exposed to this. Read the full buffer instead
  (`real-agent.ts`'s `waitUntilAgentReady` was fixed this way after it timed out on both engines with the marker sitting
  in scrollback and an effectively blank rendered viewport).
