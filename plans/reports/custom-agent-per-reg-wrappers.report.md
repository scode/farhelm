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

### Landing

#### What landed with it

Only this plan landed this round, and main had gained nothing but plan bookkeeping since its base, so nothing interacted
with it. An independent reviewer and I both read the change against the code around it:

- the helm database upgrade rebuilds the custom agent list keeping every agent's ID, definition and timestamps; nothing
  else in the database refers to that list, and an older helm refuses the upgraded database, as it does any newer one;
- the registration number that tells a re-registered agent from the old one is never reused, even after the newest agent
  is deleted, and a test checks this;
- a launch remembers its wrapper only for the registration it was started from, so a launch that reports success after
  Delete, or after the same ID is registered again, changes nothing. Every way of starting a session from the launcher
  says which registration it used. Agent-made launches, plain Replace and Restart with remember no wrapper at all. A
  new-checkout launch accepted on retry after a lost reply remembers none for a custom agent, but still does for
  built-in agents;
- the helm now sends the desktop app a new, optional list of each custom agent's remembered wrapper. The desktop app
  always runs its own helm, so the protocol version did not need to change.

#### Fixes made while landing

Two documentation fixes, both found by the independent review:

- The implementation spec still said the custom agent table is keyed by the agent's ID. Since this plan's database
  upgrade, the table is keyed by the registration number; the agent's ID is still unique but is no longer the key. The
  paragraph now says so.
- One new test's description said a launch of a second custom agent without a wrapper clears that agent's own wrapper,
  but that agent never had one, so the test cannot show it. The description now says what the test shows: the first
  agent's wrapper is left alone. Clearing an agent's own wrapper is covered by the database tests.

Nothing from the landing needs a follow-up.

#### Checks

Run on the plan's two PRs exactly as the executor pushed them, stacked on main as it stood when the landing started
(`82386429`, which had changed nothing outside the plan files since the plan's base). The landing fixes above change
only a spec paragraph and a test's description, so these runs still cover what merged. The test worker is an aarch64
Linux machine:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-gui -p farhelm-desktop`, recorded
  run d910944a-6fca-423f-ad3e-fe306d20322a: 5,928 passed, 7 skipped. That covers the helm's database upgrade from every
  older schema, the new tests of late launches after Delete and re-registration, and all the launcher and template
  tests, with both PRs applied. It is wider than the executor's last runs, which after the review fixes covered only the
  helm tests about custom agents and wrappers.
- On the test worker, the desktop app's end-to-end smoke, recorded run 7e124514-9865-4f9d-b9e5-bc730e266449: all 49
  steps passed, except the paste step, which runs only on a Mac and was skipped as always on Linux. It opens the
  launcher and starts sessions from it. It and the screenshot comparison below are the two checks the executor's report
  skipped.
- Locally, the full screenshot golden comparison, recorded run 6f52adb8-8bd5-4664-9b95-3c0a3fdef342: no picture changed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed, and again after the landing fixes.

Reused: nothing. Skipped: the supervisor's tests and the end-to-end Rust suite, since nothing the supervisor runs or the
helm sends it changed.
