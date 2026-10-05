### What this was about

An agent can start sessions with `farhelm agent create`, `farhelm spawn` with launch flags, and `farhelm agent clone`,
and give each an idempotency key so that retrying after a lost reply returns the session the first attempt started
instead of starting a second one. Until now the host compared a retry with the first attempt by what the request
resolved to: the folder, title and launch. A retry after one of your launch templates was edited, or after a cloned
session changed, therefore resolved differently and was refused as a reused key, even though the agent repeated itself
exactly. The helm papered over this for templates by storing each keyed create's first resolution in its database and
re-sending it on a retry, but only for 30 days and 256 entries per asking session, while SPEC.md promised it without a
limit. Clone had no protection at all.

You decided in planning to compare the request as the agent sent it instead, for clone too, and to delete the helm's
stored-resolution machinery entirely. Because nothing on the helm remembers which host a key went to any more, you also
decided that a keyed `farhelm agent create` must name `--host`, and that a host name coming to mean another machine
between an attempt and its retry is an accepted gap. Finally, refusals stop quoting the key back. That is what landed:

- With every keyed create and clone, the helm sends the host a fingerprint of the request as the agent sent it (host
  name, template names in order, flags, and for a spawn its parent; for a clone the source, host and overrides as
  given). The host compares that instead of the resolved launch. A retry repeating the request returns the first session
  even after a template or the clone's source changed, with no time limit. The same key with a different request is
  still refused. `farhelm spawn --inherit-agent` keeps the old comparison, as you decided.
- The helm's table of stored resolutions and all the code around it are gone; the helm database's next schema version
  drops the table, including from installs that upgraded through v0.22.0, which shipped it.
- A keyed `farhelm agent create` without `--host` is refused with a message saying why. The agent instructions and the
  CLI help say so. SPEC.md records the host-reassignment gap.
- None of the host's answers to a keyed create quote the key any more.
- SPEC.md and SPEC_impl.md describe the new rule, and the TODO entry "Settle retry limits for template-based session
  creation" is removed.

### Things you should know

- **This changes released behavior.** v0.22.0, cut while this plan ran, shipped the agent launch flags with the 30-day
  stored resolution. Requiring `--host` with a key breaks an agent that relied on a template's host, so the second PR is
  marked breaking and carries its own changelog entry.
- **A retry is now shown and checked as what it resolves to now.** The approval card, and the rule that refuses an
  agent's YOLO launch on a host where "start YOLO sessions here without asking" is off, judge the retry's current
  resolution, not the first attempt's. Two consequences, both written into SPEC.md: a retry whose template was edited
  into a YOLO launch on such a host is refused even when it would only have returned the existing session; and if the
  first attempt was cut short before its session started (a host crash at the wrong moment), the retry is refused rather
  than started when its launch, folder or title now differs from what the first attempt recorded, so nothing starts that
  you did not see on the card. The reviewers found this; the plan did not anticipate it. For an ordinary keyed create
  that refusal sticks to the key (a new key is needed), as every refusal of a keyed create does; a spawn's key only ever
  lives as long as its child session, so with no child it is freed and the next retry creates afresh.
- **The card for a retry can show a different launch than the session it returns.** When the retry replays an existing
  session, the card shows what the request resolves to now, which may differ from what that session runs. Approving only
  returns the existing session; nothing new starts. This is accepted, not fixed.
- **Folder history after a replay.** When a retry gets an earlier session back, the helm now records that session's own
  folder in your folder suggestions, not the one the retry resolved to. A reviewer found this.
- **Upgrade.** A retry that spans this upgrade, under a key from before it, is refused as a reused key, never
  duplicated, as with the earlier change that removed profiles.
- **Protocol version.** Supervisors and helms must be the same protocol version again (43). Another plan (session
  notifications) was being landed as this one finished and may take that number first; if so, the landing renumbers this
  one, which is mechanical.

### Open questions and possible follow-ups

- None required. If the card mismatch on a replayed retry ever confuses anyone, a follow-up could have the host answer
  "this key already has a session" before the card is shown, at the cost of an extra round trip.

### The PRs

- #1617 feat: let a host match an agent's retried create by its request — the protocol field, the host's comparison and
  the refusals around it.
- #1627 fix!: match agent create retries by the request, not a stored launch — the helm side, the table's removal, the
  `--host` rule, specs, changelog entry, TODO removal.

### Checks run, reused and skipped

- Run, on the final code before the last rebase: `cargo nextest run` over the protocol crate, the `farhelm` binary's
  unit tests and its `agent_cli` and `spawn_cli` CLI tests, the end-to-end idempotency tests, the supervisor's create,
  idempotency and retry tests, and the helm's agent relay, store, template-schema and session tests (run `e2bc8cab`, 811
  passed). After rebasing onto the latest main, a narrower selection of the same areas (run `cb2e0b6f`, 535 passed).
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D
  warnings`, `dprint check`, the test-sleep checker (0 unannotated delays) and
  `python3 releasing/check-changelog.py
  format`.
- Reused: nothing from before this plan. What landed on main during the plan (restart hints, the Mac app's automatic
  updates, uninstall, release notes, docs) touches none of the create, idempotency or relay code; the restart-hints
  change edited a different paragraph of the agent instructions.
- New tests: the host compares the fingerprint and not the launch, refuses a malformed one and one without a key, and
  refuses to restart an interrupted attempt whose launch, folder or title changed (with a control that an unchanged
  retry is not refused); the helm sends the same fingerprint across a template edit and across a clone source change, a
  different one for a different request, none without a key; a keyed create without `--host` is refused even when a
  template supplies a host; the fingerprint's encoding is pinned; folder history after a replay; the schema step that
  drops the table.
- Skipped: the browser end-to-end suite and desktop checks (no UI change), and the installer and CentOS provisioning
  checks (unaffected). The full workspace battery was not run; the selections above cover every crate this changed.

### Review gate

Each PR was reviewed by Claude Opus 5.5 and gpt-6-astra at high effort, as you required, and the stack had a second
round with both after the fixes. Round one: on the first PR, gpt-6-astra found a test that did not check its own
premise, and Opus found three more host messages quoting the key, a fingerprint without a key being silently ignored,
and docs to correct. On the second, gpt-6-astra found the folder-history problem above, and Opus found the problem of
restarting a cut-short first attempt with what it recorded, and asked for a pinned encoding and sharper tests. Round
two: gpt-6-astra found that the check on restarting a cut-short attempt ignored the folder, and Opus found a stale
SPEC_impl paragraph about clones and asked for the title to be compared too. All of it was applied. The one deliberate
exception is an internal error message deep in the host's database code, which still includes the key because it only
appears if the database breaks an invariant.

### Landing

Landed on 2026-10-05 (UTC) as two squash commits on main: #1617 (the host compares a retry by the request the agent
sent) and then #1627 (the helm side: the stored resolutions removed, `--host` required with a key, the specs). Nothing
else reached main while they merged.

#### What else was on main

Between the commit the stack was built on and the landing, the session notifications plan landed (the bell on the
sidebar row). It changes several of the same files, including the helm's database, and that is where the landing had to
step in.

#### A fix made while landing: the helm database version

Both plans added a step to the helm's database upgrade and both numbered it 41: session notifications to add its table,
this plan to drop the table of stored resolutions. Session notifications landed first, so its step is 41 on main. Had
this plan landed with its own step also at 41, a helm whose database had already reached 41 through session
notifications would have skipped the drop and kept the table of stored resolutions forever, and the code that checks a
new database against the expected layout would have disagreed with an upgraded one. The landing renumbered this plan's
step to 42, after the notifications step, and made a new database at 42 carry the notifications table and not the
dropped one. It updated the step's test (renamed, and rewound to 41 rather than 40 so it exercises the drop step on its
own), the comments that name the version, and the sentence in SPEC_impl.md that said schema 41 drops the table. It also
corrected the test's explanation of why the table can still exist: the stable v0.22.0 release shipped it, not only a
release candidate. The protocol version needed no change: session notifications did not change it, so this plan's 43
stands, and the report's caution that the landing might have to renumber it did not apply.

The renumbering went into #1627, the PR that adds the step, before anything merged. A separate reviewer that had not
seen the work listed every place the version appears on both sides before the merge; the landing's changes cover that
list. The same reviewer found no other interaction with session notifications: the fields each side added to shared
records are filled in wherever the other side builds those records, the changes to the supervisor's create and relaunch
handling are in different places, and the two sides' SPEC.md and SPEC_impl.md edits do not contradict each other. It
also found nothing outside the two PRs that still uses the removed stored resolutions, expects a refusal that quotes the
key, or runs a keyed `farhelm agent create` without `--host`.

When the cli-permission-prompts plan landed on 2026-10-05 (UTC), shortly before this one, its landing notes said this
plan contradicted it in three places and needed revising before it ran: its plan said `farhelm spawn --inherit-agent` is
answered by the session's own supervisor and never the helm, it compared `--confirm-yolo` as part of a retry, and it
assumed protocol version 40. The report above shows the executor worked against the design that landed: it describes
retries as the approval cards and the agent YOLO rule now judge them, and the protocol is at 43. On the other two, the
code settles `--confirm-yolo`, since that flag no longer exists anywhere and the stack compiles; the report says only
that `--inherit-agent` keeps its old comparison on the host, and neither the report nor this landing says more about who
answers it. In the code that landed, every `farhelm spawn` goes through the helm, as cli-permission-prompts made it.

#### Checks

- Run now, on the final stack after the renumbering, through the test-run recorder: the executor's own selection (the
  protocol crate, the `farhelm` binary's unit tests and its `agent_cli` and `spawn_cli` tests, the end-to-end create
  idempotency tests, and the supervisor's create, retry and reservation tests), widened with the helm's unit tests in
  full, the end-to-end structured launch tests and the supervisor's notification tests: run `9a4cd7fc`, 1500 passed. The
  helm's tests include its whole database upgrade ladder and the renumbered drop step.
- Also run, after the last comment fix: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check`, `python -B scripts/check-test-sleeps.py` (no
  unannotated delays), and `python3 releasing/check-changelog.py format`. All clean.
- Reused: nothing; the run above covers every test selection in the report.
- Skipped: the browser suite and desktop checks, for the report's reason (no UI change).

The report's "the helm database's next schema version drops the table" now means schema 42; session notifications
took 41.
