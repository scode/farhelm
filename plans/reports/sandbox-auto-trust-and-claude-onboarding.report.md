# Sandbox workspace trust and Claude Code first-run setup

## What this was about

A recipe host could have valid Claude Code credentials and still stop at the theme chooser. A new sandbox or container
session could also stop at the agent's workspace trust question unless that agent's Settings switch was on. The plan
chose to fill in Claude's missing first-run settings and apply workspace trust automatically on supported launches into
a session's own sandbox or container.

## Things you should know

Farhelm fills in Claude's first-run setting when preparing a recipe host. It also seeds trust for the home folder when
preparing a host owned by one session. Existing choices and credentials take precedence. The isolated Claude Code
2.1.332 probe needed only `hasCompletedOnboarding` to pass the theme chooser. Once home was trusted, Claude started both
there and in a checkout below it. An alternate Claude config can still prompt, as can an explicit choice in the default
file. If Farhelm cannot safely update the default file, for example if it is a symbolic link, it reports that first-run
setup is pending.

Claude Code, Codex, Muse Code, Pi and Antigravity receive workspace trust automatically when Farhelm composes a launch
into a sandbox or container owned by that session. Installed hosts and persistent recipe hosts use the Settings switch.
Antigravity's explicit `--settings` takes precedence. Restart and Replace using the original launch keep its saved
choices. Claude's separate setting for applying trust after a managed clone also remains in force.

When starting Claude, Farhelm protects the host's home folder and its parents from new trust entries. If recipe setup
already trusted the exact home folder, Farhelm can acknowledge that existing trust without writing it again or showing a
trust warning for that launch.

## Open questions and possible follow-ups

The installed Claude Code probe used a loopback endpoint because there was no usable subscription login. The first
launch with a live Claude subscription remains unverified.

## Pull requests

- Fill in Claude Code's first-run settings and seed home trust for disposable sessions.
  [#4943](https://github.com/scode/farhelm/pull/4943/changes)
- Apply automatic workspace trust to launches in session-owned sandboxes and containers.
  [#4955](https://github.com/scode/farhelm/pull/4955/changes)

## Checks

- Isolated Claude Code 2.1.332 startup probe `0d64422a-4aae-4892-9665-0a347f08c70f` used nine private homes and a
  loopback endpoint. The empty and theme-only cases opened the theme chooser. Onboarding alone reached the trust
  question. With home trust seeded and no theme key, Claude reached its prompt in both home and a child directory. The
  probe exercised startup screens without submitting a prompt.
- Reused pinned nextest checks from step 01 at `94f2473a9884`. The 44-test `farhelm-helm --lib` run selected
  `test(credential_delivery) | test(/tensorlake_add_tests::/) | test(/agent_endpoints::/) | test(conditional_settings_install)`
  (`5d7845cb-8ea6-43c8-9fc7-a18752facac7`). The 35-test `farhelm-helm --lib` run selected
  `test(/tensorlake_add::tests::/)` (`e06d5d8a-db6b-44b3-8665-882a44a432b0`). The 20-test `farhelm-gui --lib` run
  selected `test(recipes)` (`6b6da0a7-13ee-4cfb-a3c2-19dd93c5150f`). The draft retained the tested patch through both
  rebases. The final launch gate covered the integrated paths after the changes to main.
- `python3 scripts/check-gui-goldens.py --update` passed for both steps (`d9ba19eb-a21a-4d69-a22f-26f727a516d6` and
  `02a3c682-d39b-4138-88af-770eb7b423eb`). Reviewed and published the light and dark recipe summary in step 01, then
  both appearances of the five agent Settings pages in step 02. The full catalog captures showed only those intended
  differences.
- On the rebased stack, pinned `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib` passed all 394 selected
  tests (`45fc7f2c-3fb1-4ba7-92e9-a24f044309e8`). The selection covered the launch policy, provider routes, credential
  delivery, and the exact-home Claude trust guard. Pinned
  `cargo nextest run -p farhelm-gui --lib -E "test(/settings::/)"` passed all 261 selected tests
  (`5c9610a5-b1e2-4a4d-9157-4661f8247af0`). Those two runs covered the final source tree before publication updated the
  ten screenshot references. Both `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` passed.
- The docs site built and checked links. Rust formatting, the Rust test-delay check, changed Markdown formatting,
  changelog format and whitespace checks passed. The full runtime battery and live provider launches were skipped
  because the changed launch paths are covered by focused local tests and scripted provider fixtures.

## Review

The independent source review gate passed with no remaining findings. The reviews confirmed the minimal Claude settings
write, how it preserves existing choices and the destination-based trust rule. The first review found a missing fixture
for routing settings differently on persistent and disposable hosts and Settings copy whose scope was too broad. The
second review verified the expanded fixtures, revised wording and the rebase over managed checkout history.

### Landing

#### What landed with it

Only this plan landed this round, through its two PRs and the landing fix
[#4958](https://github.com/scode/farhelm/pull/4958/changes). Since the plan's PRs were written, main gained only plans
queue commits, so nothing could interact with it. It builds on trust work that landed in the previous rounds: when you
approve an agent's request for a fresh checkout, Claude's trust setting is the one in effect when the card appeared; a
trusted Antigravity launch uses exactly the folder its card shows; and Farhelm skips automatic Claude trust when
Claude's settings folder is set to a relative path. The landing's review read this plan against that work and found no
conflict.

This plan widens automatic workspace trust on purpose, so the review checked the boundary first, and it holds:

- **Where automatic trust applies.** Only to a launch into a sandbox or container that the helm registered as that
  session's own. That is fixed when the host is registered, so neither an agent nor a remote host can claim it or change
  it between an approval and the launch.
- **Where it does not.** Installed hosts, SSH hosts and persistent recipe hosts still follow each agent's Settings
  switch.
- **Saved launches.** Plain Restart, and the Replace answers that reuse the saved command, keep the saved launch's
  choice.
- **Claude's home folder.** When starting Claude, the supervisor only reads Claude's settings to accept an existing
  trust entry for exactly the home folder; it never accepts or writes one for `/` or any parent of home. The seed that
  recipe setup writes uses the real home folder, refuses `/`, adds only missing settings, keeps an explicit "no", and
  refuses to replace a symbolic link or anything but an ordinary file.

No database schema or protocol version changed. The review counted no blockers, two should-fix items and five small
points. The first should-fix is fixed below with three of the small points; the other should-fix and two small points
are in the last section, each marked.

#### Fixes made while landing

- **The approval card said the trust came from Settings when it did not.** When an agent asks to launch into its own
  sandbox or container, the card's Trusted workspace text read "Enabled on the agent's Settings page", even with every
  agent's switch off. A user approving the launch who checked Settings found the switch off and could not tell why the
  launch was trusted. #4958 changes the text to name both sources (the agent's Settings switch, or automatically in a
  session's own sandbox or container), amends SPEC.md's description of the chip to match, and updates the test that
  pinned the old wording. No screenshot shows such a card.
- **Smaller corrections in the same PR.** The plan's changelog draft for the trust change moves from "fixed" to
  "changed", since users who never turned trust on now get it in these sessions; a SPEC_impl.md sentence that still
  called the Antigravity switch "all-hosts" while limiting it to installed hosts drops the word; and a test helper's
  description now says which hosts it covers and where each gets its trust.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`66c6f4aa`), which had gained only plans
queue commits since the plan's base, so the rebase applied without conflicts; the rebased stack's top commit was
`4eb85f1e`. The only later change is the landing fix [#4958](https://github.com/scode/farhelm/pull/4958/changes), which
rewords the approval card's trusted workspace text and its spec sentence and makes the smaller corrections the plan's
notes list; apart from the card text, those change only documentation, the changelog draft's kind and a test helper's
comment. After it I ran the GUI's approval card tests again,
`cargo nextest run -p farhelm-gui --lib -E 'test(/approvals::/)'` on the test worker (recorder run
`ef15d53d-b23c-4ecb-a9b4-6d4bf4568c75`, all 95 passed), and the formatters, the changelog check and the instruction
check below, which all passed. That final tree is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `fe614d1d-cc6f-4758-abf1-8eaa60772cd2`): all 8667 tests passed, with no runtime skip. The plan changed the helm's
  provisioning, credential delivery, session creation, agent requests and workspace trust policy, and the supervisor's
  Claude trust guard and launch, with their tests; the whole suite was the narrowest run that covered them and their
  callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `6b12fc91-4129-45e2-bb6e-ecc903e18ecd`): all 184 passed. The desktop app builds the GUI crate this plan changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `43a3e0dd-58af-4c72-aaca-bc96e803a9e7`): 50 steps passed, one skipped by design on Linux (paste). It opens Settings,
  whose agent pages the plan reworded.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `d1ff4090-b00d-4a67-87bc-d6908f8d43e7`): passed, including the ten references the plan updated (the recipe summary and
  five agent Settings pages, light and dark). No screenshot shows an approval card with trust, so the landing fix
  changes none.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. The plan
  changed seven docs pages.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main),
  `python -B scripts/check-test-sleeps.py` and `dprint check`, locally: passed.

Reused: the executor's isolated Claude Code 2.1.332 startup probe (recorder run `0d64422a-4aae-4892-9665-0a347f08c70f`),
which showed which first-run keys Claude needs; nothing in the settings the plan writes changed since. The executor's
other runs were focused selections that the runs above cover again.

Skipped: a launch with a live Claude subscription, which the report also leaves unverified; live sandbox and container
launches, which need provider accounts; the workspace doctests, since no documentation example changed; a Mac run, since
nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing. The report does not describe the approval card's wording.

#### Things you should know that the report does not say

- **(Should-fix) On a long-used persistent recipe host, "Claude Code first-run setup is pending" may show on every
  reconnect.** The one-time first-run setup reads the host's Claude settings file before adding the missing setting.
  That file grows by an entry for every folder Claude has started in; if it is over 1 MiB Farhelm refuses to read it,
  and close to that size the read may not finish within the setup's 10 seconds. Either way the setup is never marked
  done, so the notice comes back each time, although Claude on that host finished its own first-run setup long ago.
  Hosts with many managed checkouts are the likely case; the review is sure of the code path but not of how often hosts
  get that big. The fix is a small design choice (mark the setup done when the file is too large to read, or check for
  the setting on the host first instead of reading the whole file); say if you want a TODO entry.
- **(Small) No test covers an agent's own requests inside its sandbox.** Launches you start in a session's own sandbox
  are tested through the helm's API. Requests an agent makes from inside its own sandbox or container (create, spawn,
  checkout) are not, and no such test uses a container host; the code for them reads correctly, and the decision it
  rests on is tested. I recommend adding one; say if you want a TODO entry.
- **(Small) Farhelm's first-run edit does not take Claude's own file lock.** The supervisor's trust editor waits on the
  lock Claude uses for its settings file; the one-time first-run setup does not. If Claude wrote the same file at the
  same moment, one of the two writes could be lost. The edit runs once per host, while the host is being set up, so a
  clash is unlikely, and SPEC_impl.md already accepts the same gap for Claude's `settings.json`. A sentence in
  SPEC_impl.md saying so would make it explicit; say if you want it added, or a TODO entry.
