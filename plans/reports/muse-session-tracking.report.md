# Muse session tracking

## Since the last review

The conversation-resume PR, #2788, now includes the three missed test updates and the corrected Restart with explanation
requested by the latest Decision. Older Muse launches without a resume command are told to use Clone or Replace with.
The end-to-end fixture now supplies Muse's real resume selector and expects tracking to wait for a report; the launcher
test expects Muse capture and uses Cursor to check the warning's layout. The other three PRs are unchanged apart from
rebasing onto main, including the landed Trash work. The capture PR also records the pre-existing WebKit focus failure
in FLAKES, TODO and the deflake exclusion list, as the repository requires for established latent flakes.

## What this was about

Muse could run in a Farhelm session, but Restart could not resume its conversation. This stack adds conversation resume
through resident wrappers and reads Muse's working, idle and directory-trust screens. The earlier Decision accepts that
smaller scope and defers recognizing questions and approvals during a conversation. The original TODO now names only
that deferred work; the separate TODO recording the scope judgment remains.

## Things you should know

Muse 1.4.14 is the minimum verified version. Farhelm supplies hooks per launch without installing a plugin or changing
Muse's configuration. Private offline-echo runs verified conversation identifiers, start and end-of-turn reports,
resume, and the start hook's instructions announcement. Capture shares Claude's positional attribution and conversation
pin, including correction when a parent reports after its child.

General conversation questions and approval dialogs remain unrecognized: **a Muse session can show Idle while Muse waits
for an answer**. SPEC and the Muse documentation state this limitation. No further provider investigation, sign-in,
operator credential use or saved-configuration changes were made in this correction round.

Existing stored Muse launches gain the integration on their next start but keep their absent resume command. Recreate
them through Clone or Replace with to gain resumable launches; there is no migration or backfill. The peer protocol is
now 65, so update the helm and supervisors together. The support policy permits dropping an older agent version two
weeks after a newer release, without adding compatibility code.

The changed documentation pages are Muse, Agent wrappers, the agents support matrix and Agent hook injection. No preview
server was started during unattended execution. The four PRs remain drafts for the landing monitor.

## Open questions and possible follow-ups

Recognizing conversation questions and approval dialogs needs a supported authenticated test provider or documented
fixture protocol that can make Muse ask a real question, followed by real screen captures. It remains in TODO and is
outside the approved scope. The maintainer can revisit the smaller-scope judgment through its separate TODO entry.

Two existing open Rust flakes recurred: the shallow-pause terminal reset assertion (FLAKES, October 9 and 17) and the
hung-save deadline assertion (October 22). Both passed an isolated retry. WebKit's Restart with focus check also failed
and passed an exact retry; it is now recorded in FLAKES and TODO. Their causes remain unresolved, the assertions were
left intact, and the failed runs remain failures.

## The PRs

- [#2785 — agent version support policy](https://github.com/scode/farhelm/pull/2785/changes).
- [#2786 — Muse's own integration kind](https://github.com/scode/farhelm/pull/2786/changes).
- [#2788 — Muse conversation resume and landing corrections](https://github.com/scode/farhelm/pull/2788/changes).
- [#2789 — Muse working, idle and directory-trust status](https://github.com/scode/farhelm/pull/2789/changes).

## Checks run, reused and skipped

The requested `cargo nextest run --workspace --exclude farhelm-desktop` completed with 4,509 runtime passes, two
failures and 40 skipped tests, run `a5463c08-8bbb-4f54-9da7-322c4bd92be2`. JUnit additionally counts successful fixture
setup. The failures were the two existing flakes above; the corrected Muse cases and the wrapper, capture and status
tests passed. An exact two-case retry passed, run `da530b61-ca5a-4db3-8875-305227edfc03`, command
`cargo nextest run -p farhelm --test e2e -p farhelm-ui --lib -E 'test(=terminal_backpressure::shallow_pause_resumes_without_reset_or_replay) | test(=api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline)'`.
No full Rust rerun was made. Both records are retained; a later pass does not replace the failed observation.

The exact structured-launch and Restart with availability tests passed, run `4ac613d1-c217-43e4-9161-70ec104e1b4c`:
`cargo nextest run -p farhelm --test e2e -p farhelm-ui --lib -E 'test(=structured_launches::structured_launches_forward_to_ready_processes_and_survive_a_fresh_generation) | test(restart_with_availability_explains_each_unavailable_case)'`.
Both runtime cases and fixture setup passed; 1207 cases were selected out. The tested dirty source above `dd500983` has
fingerprint `f815ee88e1ffa1d981fede105c4f6ed5b968c05047fe41dc26be1842038c0fe5`.

All three Rust commands used Linux x86_64, nextest 0.9.148, pinned tmux 3.7c, four slots and zero retries on that same
source fingerprint. Reports and output are complete, with no runtime substrate-skip marker. Selected-out cases are not
runtime passes.

The requested `npx playwright test tests/named-conversations.spec.ts tests/restart-with.spec.ts` ran 112 cases with
three workers and zero retries, run `ca41e45d-2ea0-486b-a8b1-7f1defad7146`: Chromium 56 passed; WebKit 55 passed and one
failed. Both engines passed the corrected Muse reporting-help case. The remaining failure was the same
gap-focus/container-Enter case identified by the landing Decision. Its unchanged exact WebKit retry passed one case, run
`143efecd-81a2-4e44-8ba1-8275f8fdb7ed`, using `--project=webkit-restart-with` and the case title as grep. That generic
recording has complete output and cleanup but does not claim strict browser-mode count validation. An earlier anchored
grep selected no tests, run `53e946eb-c1fc-49c2-aed1-18b75e3f8d9f`; it is retained and supplies no assertion evidence.
The selected browser run has complete validated reports, no skips or unstarted cases, full output and clean teardown.
Both browser observations used the same source fingerprint above, pinned tmux and Playwright 1.64.0. Ordinary retained
desktop/phone screenshots were inspected; the warning and input fit and wrap within the launcher.

Rust formatting and the source sleep checker passed for the corrections: 322 delays, none unexplained. Repository dprint
does not include the changed TypeScript spec; its no-files exit was not counted as a pass.

`cargo clippy -p farhelm-ui -p farhelm --all-targets -- -D warnings`, `cargo build`, and the Dioxus 0.7.10 release web
build passed. The initial `dx` invocation selected Deno's unrelated command; selecting the installed pinned Dioxus
binary resolved that build-tool collision without changing source or configuration.

The final flake record is documentation and exclusion-list bookkeeping only. `dprint check TODO.md FLAKES.md` passed;
the earlier runtime and Clippy evidence remains applicable because no executable, fixture or browser-test source changed
after those runs. No further runtime tests were launched for that record.

The previous website build and internal-link validation passed, run `acada2f8-c1e8-424c-98ad-e5ef25f4b717`, building 32
pages with Node 26.12.1 and Bun 1.4.2. It is reused for the unchanged Muse documentation: this round changes no website
source, and the upstream Trash documentation changes do not interact with these pages. Prior shipped-CLI-bin Clippy,
changelog formatting and agent-instruction checks remain applicable to unchanged inputs.

Desktop, installer, provisioning and the full browser battery were skipped because these corrections change ordinary
restart wording and test premises; the requested two browser specs exercise the affected browser behavior. No additional
vendor or real-provider question run was made because question recognition is explicitly deferred.

## Review gate's outcome

All four units passed the required single fresh-context gpt-6.1-sol/high gates in the implementation rounds. The
complete amended conversation-resume PR passed a fresh review on that same model and effort with no actionable findings.
The other units retain their reviewed patches. The independent resume check and the report's required cold read passed.

### Landing

Landed on the second delivery, with one test assertion added at landing. Muse sessions now resume the right conversation
through the launch wrappers and show working, idle and directory-trust status read from Muse's own screens. As decided
on your behalf, recognizing Muse's questions and approval dialogs is deferred: until it lands, a Muse session waiting
for your answer can look idle. SPEC.md says so, and the plan's TODO entry is narrowed to that part.

- #2785: a support window for older agent versions.
- #2786: Muse as its own agent kind.
- #2788: resuming Muse conversations.
- #2789: reading Muse's status.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the Trash footer (#2787) and TODO entries; nothing interacted. The first landing
attempt found three existing tests still expecting Muse to be an agent Farhelm does not track, and a Restart with
message that blamed a missing conversation when the real problem was a missing resume command. The redelivery fixed
exactly those: Restart with now tells an older Muse session without a resume command to use Clone or Replace with. A
fresh-context review of the whole plan found no database change, the protocol version raised for the new Muse values,
Claude's behavior unchanged and the per-agent code rules followed. The protocol change means the helm and its
supervisors must run matching versions, which the protocol check enforces. A second review of the redelivery found the
fixes carried out with no test weakened, and one lost test check, restored at landing (below).

#### Fixes made at landing

- Rebuilding the Restart with test's Muse case had removed its only check that an agent launch whose agent has no
  conversation reporting (Cursor or OpenCode) is told "<agent> sessions can't be resumed". The landing added that check
  back, for Cursor, to #2788's test.

#### Things you should know

- The executor recorded the WebKit focus failure in the Restart with tests as an open flake (flake log, TODO.md's
  Deflake list and the sweep's exclusion list), from its own failing run `ca41e45d-2ea0-486b-a8b1-7f1defad7146`; the
  same check had failed at an earlier landing of another plan and passed on rerun.
- Muse versions older than 1.4.14 will be started with Farhelm's hook configuration passed on the command line, which
  nobody tried on those versions; if one rejects it, that session will not start. The plan's new support window for
  older agent versions covers this.

#### Possible follow-ups

- Restart's own tooltip, for an older Muse session without a resume command, still says "this command launch did not opt
  into Resume … Replace starts it over": it is an agent launch, and the text does not mention Clone.
- The documentation for captured agent screens and AGENTS.md's "Agent screens" flow describe status reading and
  re-capture as Claude and Codex only. Muse's screens were captured by hand, so re-capturing agent screens after a Muse
  upgrade will not refresh them, and Muse's status reading could go stale unnoticed.

#### Checks

This round landed only this plan. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the docs
  website build, the workspace debug build and the UI's release web build: all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, run `d8d82520-09c7-4055-89a9-0f897c527c64`: 4510 passed, 40 skipped by design, and 1 failed: the known flake
  "shallow pause resumes without reset or replay", in terminal code this plan does not touch. The UI's Rust tests,
  `cargo nextest run -p farhelm-ui`, run `c9bbee9d-f168-4095-94c4-fb57ed9280b7`: all 805 passed. The UI's JavaScript
  tests (`node --test` in `crates/farhelm-ui/js-tests`), run `5849ba43-7648-4bc9-8bbc-729554294f9b`: 358 passed.
- `npx playwright test` in `e2e/` with `tests/named-conversations.spec.ts` and `tests/restart-with.spec.ts`, on Chromium
  and WebKit, three workers each with its own helm and supervisors, no retries, run
  `e1c5935e-6592-4e13-ba50-9b7042ffb001`: all 112 passed.
- After the landing fix: `cargo fmt --all -- --check`, and the changed test alone,
  `cargo nextest run -p farhelm-ui -E 'test(=session_view::tests::restart_with_availability_explains_each_unavailable_case)'`,
  run `e5565ef8-2866-413c-a8e4-2ca8203e4e55`: passed.
- Reused from the first landing attempt, on the plan before this delivery's test and message fixes: the workspace-trust,
  guarded-permissions and Goose and Pi launcher browser specs, which all passed within run
  `9f76f468-811e-4b98-9635-6cb2b6d1d86c`. That run failed overall, only in the named-conversations and restart-with
  specs, which ran again above. Skipped: the rest of the browser suite (the UI changes are Muse's agent kind, its mark
  and one Restart with message), the desktop app checks (no desktop code changed) and real Muse runs beyond the
  executor's captures.

The executor also ran the whole Rust workspace; this landing ran it again on the final version, with the affected
browser specs. Apart from the PRs no longer being drafts, nothing in the executor's report became untrue at landing.
