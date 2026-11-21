# Podman container triage

## What this was about

This plan worked through 18 decisions about Podman containers. It fixed seven product or documentation findings and
strengthened five tests. Three findings went into two Plan follow-up entries, two test findings needed no change under
the agreed scope, and one report was refuted by the current sources.

## Things you should know

- New containers read the container host's current proxy settings for their first start. Builds keep the probe's proxy
  observation. An idle connection to a stopped container could keep the connection manager alive after its owner
  released it. The manager can now shut down normally. When the container's Farhelm exits before connecting, diagnostics
  identify the process exit so the operator has a specific failure to investigate. The dial documentation now describes
  the provider check before opening the connection. [#4482](https://github.com/scode/farhelm/pull/4482/changes)
- Looking up the login account in a custom base image works without a usable home directory for root. Removing either a
  recipe container or a retired container asks Podman to remove its unused anonymous volumes.
  [#4490](https://github.com/scode/farhelm/pull/4490/changes)
- A recipe Delete that discovers an interrupted image capture can offer **Delete anyway** after Podman reports the tag
  absent. The prompt warns that an image may still appear on the container host. At confirmation Farhelm checks again,
  and an image that exists goes through the usual ownership and usage checks. The new question is in Settings, in the
  area assigned to the desktop triage plan. [#4501](https://github.com/scode/farhelm/pull/4501/changes)
- Five test changes make the checks observe a duplicate probe as soon as it is queued, the exact proxy values sent
  privately, the selected login and installation already saved at create dispatch, the settled connection failure after
  a failed automatic start, and exactly one host in discovery from inside a container. The last check touches only the
  Podman module of the end-to-end suite, in the supervisor triage plan's area.
  [#4515](https://github.com/scode/farhelm/pull/4515/changes)
- The alleged unregistered Podman test module was discarded because the build module explicitly includes it, and tests
  from it ran. The SSH fixture may miss a builder stalled before normal Podman inventory can see it. When that happens,
  the test still fails and the recorder retains its failure. The triage decision limited harness changes to crashes and
  loss or misattribution of evidence, so this cleanup-only gap was left as it was. The real Delete test still checks
  both outcomes, keeping a container with sessions and removing it after its last session. Proving which Farhelm
  supplied the remaining count would need a way to force the helm and container to see different session lists, beyond
  the approved small test changes. [#4515](https://github.com/scode/farhelm/pull/4515/changes)

## Open questions and possible follow-ups

- **Keep working container images when Rebuild fails.** Rebuild currently retires its usable images before building
  replacements. The follow-up preserves them until the replacement succeeds, while launches during a Rebuild still share
  that Rebuild's result. Coordinating those paths took more than the size approved for this fix.
  [#4490](https://github.com/scode/farhelm/pull/4490/changes)
- **Apply host identity rules to Podman places.** The paired decisions require Farhelm to refuse twin container hosts on
  the same machine and account, and to verify that a container is absent on the host where it was created before
  forgetting it. For an older row with no recorded original identity, absence can mean a deleted container or an SSH
  alias now reaching a different account or machine. The two fixes share one follow-up so it can settle safe recovery
  for older rows and keep support for places without a supervisor. Retaining these rows preserves a way to find
  containers that may still be running, but can leave stale entries after out-of-band removal. An explicit forget action
  could clear those entries while risking loss of management of a live container. That policy belongs in the follow-up.
  [#4504](https://github.com/scode/farhelm/pull/4504/changes)

## The PRs

At delivery, all five PRs are open drafts awaiting the landing monitor.

- [#4482](https://github.com/scode/farhelm/pull/4482/changes) · Proxy refresh, stopped actor lifetime, dial diagnosis
  and dial documentation. Four fixed.
- [#4490](https://github.com/scode/farhelm/pull/4490/changes) · Login lookup, anonymous volume cleanup and the Rebuild
  follow-up. Two fixed, one deferred.
- [#4501](https://github.com/scode/farhelm/pull/4501/changes) · Recipe Delete after a capture with an unknown outcome.
  One fixed.
- [#4504](https://github.com/scode/farhelm/pull/4504/changes) · Follow-up for deletion after an SSH alias retarget and
  duplicate container host admission. Two deferred together.
- [#4515](https://github.com/scode/farhelm/pull/4515/changes) · Regression checks. Five tests strengthened, two
  unchanged, one report discarded.

## Checks run, reused and skipped

- **Ran the final regression selection on `a7b5b87b`.** The four selected Podman and connection-manager tests passed
  with `cargo nextest run -p farhelm-helm --lib` (recorder `91cef086-7a89-41d2-87a6-48c245f1c03a`). The four nextest
  filters selected `cold_build_sends_exact_proxy_values_only_as_private_input`,
  `lost_session_create_records_login_and_refuses_an_automatic_second_run`,
  `refresh_is_background_deduplicated_and_route_bound`, and
  `stopped_autostart_is_owned_once_and_retains_failure_across_unanswered_reads`. The real rootless Podman lifecycle
  passed with
  `cargo nextest run -p farhelm --test e2e -E 'test(=podman_environments::real_local_environment_lifecycle_and_owned_image_cleanup)'`
  (recorder `ae1d7c17-042c-461d-b30c-1000e84e3e2e`). Both ran without runtime skips. The final stack tip is `5c75d109`.
  Its product and test code for these cases matches the tested revision. The relevant tooling changes affect the
  same-source payload's build directory and where nested recorders keep evidence. The local fixture built its own
  payload and did not use nested recorders. Formatting, the Rust test-delay audit, queue invariants, instruction and
  changelog checks, and the five batches' exact archive moves were checked. The instruction checker found one existing
  archive citation warning and zero errors.
- **Reused connection and removal checks.** The focused proxy and connection selection passed on `7f92031f` (recorder
  `d8db6e72-8c0f-4189-8bd8-77be0561aff5`). The unchanged probe, stopped actor and connection tests passed at `05dbeb16`
  (`499fc3f7-e977-4bec-923e-b14aa12d30e7`). The Podman provider and legacy supervisor removal selection passed 33 tests
  at `754cfeba` (recorder `dc39d454-23df-4cf4-a8dc-d11582c03bec`). The rebase kept their implementation diffs and
  incorporated the landed spec and desktop-session changes.
- **Reused recipe Delete checks.** The focused protocol, helm and GUI selection passed 22 tests at `9e0902c7`
  (`ff154012-8fd1-45d8-bdc6-01ac6967128d`). The final editor selection passed 14 tests on `00b41e50`
  (`d714af6c-021c-47be-8025-56e223e6268e`). The four-process Linux capture at `00b41e50` produced 350 images
  (`dd0f25e1-8c66-42c2-a9c2-0022c4a9aff1`). All 348 existing references matched, and the two additions were
  byte-identical to the reviewed light and dark images. The rebase kept the existing main manifest entries and these two
  additions. The landed GUI changes affect session surfaces, while the new images show the recipe editor.
- **Skipped other runtime checks.** The identity follow-up is bookkeeping. The ignored localhost SSH acceptance is
  opt-in. The focused selections and local lifecycle cover the changed behavior, so the full runtime suite was skipped.
  The desktop change uses the recipe editor and affected no docs-shot crop or pointer targets.

## Review gate

Each PR passed an independent high-effort Sol review. Findings led to a narrower connection test and proxy handling,
deferring Rebuild's image preservation, retaining Podman's exact absence observation across the Delete reply deadline,
clarifying the host-identity follow-up, and making the pre-send test fail if any expected saved fact is absent or
different. Opus reviewed the recipe Delete screenshots and accepted the focused keyboard-focus and warning-copy
rerender. Each PR message was read independently before submission.

### Landing

#### What landed with it

The plan's five PRs merged with a landing fix, #4522, as the second of four plans landed together in this round, after
the helm triage and before the exited-terminal scrollback fix and the status-mark hover change. Since the executor based
the plan, main had gained the per-agent Settings and hook setup plan, which redrew the Settings sidebar; that made the
plan's two new pictures stale (below). The helm triage touches no Podman, recipe or connection code, and the other two
plans share no code with this one.

The 18 triage outcomes were graduated by the plan's own commits. The review queue, its index and the outcome ledger
conflicted with the helm triage's edits to the same files; the landing kept both sides by replaying each commit's own
change onto the combined files, and a script then confirmed the queue, index and ledger agree. One cosmetic effect: in
the outcome archive, this plan's entries sit before the helm triage's rather than after them.

An independent reviewer read the five PRs against the plan, the triage outcomes and the specs. Apart from the stale
pictures, it found no blocker:

- **Outcomes.** All 18 were carried out as decided, and their records in the ledger say which way each went.
- **Correctness.** The proxy refresh, the volume cleanup and the Delete anyway path behave as the outcomes asked: the
  confirmation checks Podman again, and an image that turns out to exist takes the usual ownership checks.
- **Protocol.** The new Delete anyway question changes only what the desktop app and its built-in helm say to each
  other, which always come from the same build, so no protocol version bump was needed.
- **Specs.** SPEC.md's recipe Delete text matches the app's wording and its Cancel default.

#### Fixes made while landing

All in [#4522](https://github.com/scode/farhelm/pull/4522/changes):

- **Stale pictures.** The two pictures of the recipe Delete anyway question (light and dark) were captured before
  Credentials moved into the General group of Settings, so they showed the old sidebar and failed the golden check. They
  were captured again on the combined code and published, and the scene was added to the screenshot scene list, whose
  own test refuses a picture it does not list.
- **A lint.** A new test cloned a value only to compare it as a one-element list, which the lint run refuses.
- **A false test comment.** The real-container Delete test's comment said a helm that counted its own session list would
  fail it. The triage finding showed that it would not, and the plan closed that finding with no test change, so the
  claim is removed from the comment.

#### Checks

The round landed four plans together: the helm triage, the Podman triage, the exited-terminal scrollback fix and the
status-mark hover change. The landing ran their checks once, on all four stacked on main. The Rust workspace suite, the
desktop app's tests, the lint runs and the end-to-end smoke test ran on a separate arm64 Linux test worker. The goldens,
the website build and the small checks ran locally on x86_64. The workspace run used four build jobs, as in the last
round, since runs with more had run out of memory on the worker; this round it had no memory or disk trouble.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `8ba93f1d-d933-483b-a90a-eeac88311d3a`): all 8252 passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `890a2d33-0c71-410b-9498-3a792708a974`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker, not through the recorder: failed on one lint in a new
  Podman test, fixed in #4522.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `c732c6cc-ba70-4b34-853a-fe7961b6d297`): every
  step passed but paste, which skips on Linux as it always does.
- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `2372fd39-11ef-4468-84d3-f1c70cdf9f7e`): failed only on the two pictures of the recipe Delete anyway question, which
  the Podman plan had captured before main's Settings sidebar changed. Fixed in #4522.
- `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed because that new scene was missing from the
  screenshot scene list. Fixed in #4522.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `952eb6e8-4c50-4a08-bc89-870dd1358b6d`),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `6c988518-9a04-4428-b87d-f5a1d49ede50`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `430497e6-e6fd-4d35-8648-4082a3b33d33`), the website build (`bun install --frozen-lockfile && bun run build`) with its
  link check, `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment),
  `dprint check`, `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fixes (#4521 to #4524):

- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `6d8fe2d8-8886-45e5-a8f7-c1902f41d10c`):
  re-rendered every picture and changed only the two Delete anyway pictures, which were then published.
- `python3 scripts/check-gui-goldens.py --scene 'settings-recipe-*' --scene 'session-row-*'`, locally (recorder run
  `9282ca4e-8230-435c-8d5d-20380af3477c`): passed.
- `cargo nextest run -p farhelm-helm --lib`, on the test worker (recorder run `2b0cfd85-3205-4a06-97c1-d028a7ae46db`):
  all 1801 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and the same with `--features agent`, on the test
  worker: passed. They had not run on the first pass, which stopped at the failing lint run.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `cargo fmt --all -- --check`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

The rest of the first runs still applies after the fixes, which change only a Podman test's lint, a test comment, two
pictures and the scene list, a code comment on the helm's notices table, SPEC.md wording, the desktop parity notes and a
changelog note. Skipped: the Podman suite's opt-in real-engine and real-SSH variants, which neither triage plan needed
beyond the real Podman environment test, which ran in the workspace run on the worker (about two minutes, not skipped);
macOS, since no Mac was part of this landing and the pictures used the Linux renderer only; CI, which runs only on
demand.

#### Things you should know that the report does not say

- The fix for a container whose Farhelm exits before connecting (the connection now reports the process exit instead of
  an SSH failure) has no test that goes through a real connection attempt; only the message's classification is tested
  on its own. That was the gap the finding was about, and the outcome asked for a test "where practical" without the
  report saying why none was. A connection-manager test with a Podman container whose Farhelm exits early would close
  it.
- SPEC.md's sentence that removing a container also removes its unused anonymous volumes sits at the end of the section
  on retired containerized sessions, although it covers recipe containers too. A reader looking up recipe containers
  will not find it there.
- The changelog note for the proxy fix says new containers pick up current proxy settings "even if the session launcher
  was already open". From the triage assessment, a stale probe could be reused for as long as it was cached, not only
  while the launcher was open; release curation should check that wording.
- The "retargeted host alias can forget a live container" finding stays unfixed and is one of the two Plan follow-up
  entries the report names.
