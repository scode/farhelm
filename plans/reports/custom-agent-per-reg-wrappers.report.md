# Report: remember wrapper defaults per custom agent registration

## What this was about

The session launcher remembers the last wrapper prefix you launched an agent with (for example an `env` that sets a
profile or injects a credential), so the next New starts with it. Built-in agent types each had their own memory, but
every registered custom agent shared one slot. Launching custom agent A with a wrapper preloaded that wrapper the next
time you picked custom agent B, and launching B without a wrapper erased A's. Switching from one custom agent to another
in the launcher also left whatever wrapper was in the field, typed or remembered, so one agent's wrapper could run in
front of another's command. No earlier decision was recorded beyond the TODO entry; the plan settled the behavior: one
memory per registration, the old shared value ignored, Delete forgetting it, and a launch that finishes late never
writing it back onto a deleted or re-registered agent.

What is on main now:

- Each registered custom agent remembers its own wrapper. A successful launch of that agent updates or clears only its
  own memory and leaves other custom agents and the built-in agent types alone.
- Picking a custom agent in the launcher (its chip, a search row or a template) loads that agent's remembered wrapper in
  place of the previous agent's, the way switching between agent types already did. Picking the same agent again keeps
  what you typed. A template that names its own wrapper, or explicitly clears it, still wins.
- Deleting a custom agent forgets its wrapper. An agent registered again under the same ID starts with none, and a
  launch of the old registration that reports success after the delete changes nothing.

## Things you should know

- The wrapper you had remembered for custom agents before this release is dropped, not migrated: nothing records which
  agent it was for. After upgrading, each custom agent starts with no remembered wrapper until you launch it once with
  one. Built-in agent types keep theirs.
- Each prefix now lives with its registration in the helm's custom agent list rather than in the shared preferences.
  Telling a re-registration from the original needed a registration number that is never reused, so the helm database
  upgrades: the custom agent table is rebuilt once, keeping every agent's ID, definition and timestamps.
- One rare path remembers nothing for custom agents: when a new-checkout launch's reply was lost and the helm later
  accepts the session on a retry, it cannot tell which registration the original attempt used, so it does not guess.
  Built-in agents still remember their wrapper on that path, as before.
- A template that clears the custom agent choice (sets it to none) while one is selected also empties the wrapper field,
  because a change of registration loads the new choice's memory and "no registration" has none. No hand edit does
  exactly this, so I judged it consistent with how a registration change behaves elsewhere.
- Restart with records no launch memory, so it is unaffected; plain Replace and agent-made launches still remember
  nothing, as before.

## Open questions and possible follow-ups

- None that block anything. If you would rather keep the old shared wrapper for exactly one custom agent when only one
  is registered at upgrade time, that would be a small follow-up; the plan said to ignore it, so I did.

## PRs

- #5073 fix: remember custom agent wrappers per registration (the helm keeps each agent's wrapper and refuses late
  writes from a deleted or re-registered agent; the implementation spec; the changelog fragment).
- #5074 fix: load each custom agent's own wrapper in the launcher (launcher chips, search and templates, the product
  spec, the TODO entry, the fragment's final wording).

## Checks

Run now, through the test recorder:

- The helm crate's unit tests in full, before the review fixes: 1951 passed, 2 skipped (macOS-only real-host tests), run
  `07c59158`.
- After the review fixes, which changed only tests and how one undecodable stored wrapper is read, the helm tests
  touching custom agents, wrappers, schemas and remembered settings: 83 passed, run `eed9122c`. An earlier attempt, run
  `99aaebec`, failed one new test because its fixture host asks before YOLO launches; the test was fixed to allow them,
  as its neighbouring test does. Not a flake.
- The desktop app's launcher and template tests: 957 passed (run `d219c54d`), then 958 passed after the review added a
  test (run `c89eaddc`).
- `cargo clippy -D warnings` on the proto, helm and desktop GUI crates with all targets, `cargo fmt --check`, the test
  sleep check (0 of 306 delays without a reason), `dprint check` on the changed Markdown, and the changelog format
  check.

Reused: after rebasing onto main at `cbbed003` (the supervisor's Restart with fix, the Wayland paste fix and vendored
GUI toolkit changes), nothing that landed touches the custom agent list, preferences, launch history or the launcher's
wrapper code, so the runs above still cover the change; a type-check of both crates on the new base was clean.

Skipped: the end-to-end suite, the desktop smoke test and the screenshot goldens. The change does not alter drawing,
terminal or supervisor behavior, and the launcher path is covered by its own view tests, including one that clicks the
custom agent chips with the remembered wrapper arriving from the helm as it does in the app.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort. Neither found a blocker. Both asked for a test through the
real wiring rather than the store or draft directly: #5073 gained a test that creates custom-agent sessions through the
helm's session API and reads the remembered wrappers back, and #5074 gained a launcher test that clicks two custom
agents' chips. Smaller findings were fixed: a stricter test step, a premise check in the upgrade test, one undecodable
stored wrapper now reading as none instead of failing the whole preferences read, and docstring and wording fixes. Two
were kept deliberately: a code-structure suggestion that would have touched every way of creating a session for no
change in behavior, and the template-clearing case described above.
