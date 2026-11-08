## What this was about

Removing a host could be acknowledged before its deletion had the durability used for other trust revocations. A power
loss could restore the host with its old command and YOLO permissions. Host removal now uses the existing durability
barrier, and the implementation specification names that guarantee.

Separately, editing Custom permission words while a suspended host resumed could automatically launch the edited agent
without another Launch press. Those edits now cancel the pending press. The regression exercises an admitted edit
through the draft owner and the same identity used by the launcher's submission guard.

The other six outcomes carry out the recorded triage decisions: accept the existing right-edge Korean-sign ordering in
the specification, add two container-approval requirements to the Podman rigor TODO, and discard three findings confined
to old terminals. Each outcome has its own draft PR and execution record.

## Things you should know

The two fixes contain 37 and 99 added-plus-removed lines of code, tests and specification, within their 100-line gates;
the agreed ledger, queue and changelog exclusions apply. The launch fix adds Custom text to the existing identity rather
than introducing a separate edit-retirement mechanism. Unfinished text edits cancel too; the existing Resume connection,
automatic-name and checkout-configuration allowances stay intact.

The durability test checks that the host deletion runs under the full-sync setting and restores the ordinary setting
afterwards. It does not simulate a physical power loss. The existing macOS caveat still applies: these writes reach the
drive's cache.

The container-approval changes are recorded future work, not implementation. Launch cards must show container settings;
container-bearing cards must also use a kind older pages cannot decode, before or together with CLI container launches.

With wrapping disabled at the right edge, a Korean vowel or trailing sign printed after the last-column character can be
stored on the cell before it. Copied text then places the sign before that final character, and a later text or emoji
presentation selector still applies to the final character. Triage accepted this existing behavior; the program's input
and files are unaffected.

The three discarded findings concerned these conditional failures, assessed from source rather than observed incidents:

- A temporary startup socket-probe failure can make a live old session appear ended, so Delete stops its agent without
  the live-work warning.
- When tmux is unusable, a Delete presented as stopping nothing alive can still stop an old agent or old-tab programs
  beside an exited agent in a terminal host. The old-agent case then refuses after the destructive work; the other can
  complete.
- If a query client reports that the server exited while the server still lives, Farhelm can mistake that answer for an
  empty server. Startup can then destroy its old terminals; later listings can hide live work and allow Delete without
  the live-agent warning. Whether tmux produces that diagnostic while its server lives remains unverified.

Triage chose not to repair these old-terminal failures before removing that support. Their code remains unchanged. No
vendor code or live installation was changed.

## Open questions and possible follow-ups

The launcher's broader live-draft concern remains unverified. Its original generic investigation and later container
review remain in the feedback queue under Other; only the confirmed pending-Resume route was fixed. The two container
requirements remain in the existing Podman rigor TODO for later execution. No maintainer decision blocks this delivery.

## The PRs

All eight are open drafts in this order, each based on the preceding PR, with the first based on main.

- [#3171](https://github.com/scode/farhelm/pull/3171/changes): durable host removal, its specification and regression.
- [#3174](https://github.com/scode/farhelm/pull/3174/changes): Custom edits cancel a pending resumed launch; retain the
  generic feedback remainder.
- [#3175](https://github.com/scode/farhelm/pull/3175/changes): accept the existing right-edge sign and selector
  interaction.
- [#3176](https://github.com/scode/farhelm/pull/3176/changes): record container settings on launch cards as Podman work.
- [#3177](https://github.com/scode/farhelm/pull/3177/changes): record older-page container-card compatibility and its
  CLI ordering.
- [#3179](https://github.com/scode/farhelm/pull/3179/changes): discard the old-terminal startup-probe finding.
- [#3180](https://github.com/scode/farhelm/pull/3180/changes): discard the old-terminal Delete-consent finding.
- [#3181](https://github.com/scode/farhelm/pull/3181/changes): discard the old-terminal client-loss finding.

## Checks run, reused and skipped

- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` selecting the full-sync trust-revocation test and
  host-removal tests. Run `68554e7c-5585-4446-bc88-56518fe21081`: four passed, 1,562 selected out.
- Passed: recorder-backed `cargo nextest run -p farhelm-ui --features desktop --lib` selecting the ordinary draft-owner
  tests and guarded/resumed submission-stamp tests. Run `304b6fb8-eb66-40cb-a82d-160ac1f29217`: four passed, 1,093
  selected out. Both runs used pinned nextest 0.9.148, four slots, zero retries and `--tmux none`; the tests use a
  temporary store or headless component state, not terminals. Both have complete source capture and runner reports,
  untruncated output through EOF, no runtime `SKIPPED` marker and no cleanup limitation.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, isolated
  `python -B scripts/check-test-sleeps.py` (423 delays, none missing a reason), changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` (zero errors, three existing
  unrelated citation warnings).
- Reused for the final drafts: the store run covered base `f164d321` plus the first fix's working diff; the UI run
  covered `a8cdef3c` plus the second fix's working diff. Subsequent edits only record PR URLs and the independent
  specification, TODO and queue outcomes. Executable and test content is unchanged. All intervening main diffs through
  `23d104ac` were read: plan bookkeeping and the OpenSSH path-refusal stack, including separate Tensorlake specification
  clauses, do not interact with the store barrier or launcher identity. No rebase or runtime repetition was needed.
- Skipped: other Rust suites, browser and desktop-application runs, installer, provisioning and cloud checks. The
  focused tests cover the changed persistence and pending-submission contracts; the other six outcomes change only
  documentation and queue bookkeeping, for which runtime tests are explicitly unnecessary. No broader coverage gap was
  identified.

## Review gate's outcome

Every PR passed its required single fresh-context gpt-6.1-sol review at high effort, with no actionable findings.
Separate cold readers checked the commit and PR wording; the independent resume check passed. No review finding remains
unresolved.

### Landing

Landed as delivered, with no fixes at landing. Two PRs change code:

- #3171: removing a host now gets the same power-loss durability as other trust revocations, so a crash right after
  removal can no longer bring the host back with its old command and YOLO permissions. With its specification sentence,
  test and changelog entry.
- #3174: editing the Custom permission words while a suspended host resumes now cancels the pending Launch press,
  instead of launching the edited agent without another press. With its test and changelog entry, and a narrowed
  launcher feedback item that keeps the part still unverified.

The other six carry out the triage decisions of 2026-11-07, with no code change:

- #3175: the specification accepts an existing terminal behavior. With wrapping off at the right edge, a Korean vowel or
  trailing sign printed after the last-column character is stored on the character before it, so copied text puts the
  sign one character early. The program's input and files are unaffected.
- #3176 and #3177: two container-approval requirements added to the Podman rigor TODO entry. Launch cards must show the
  container settings, and cards that carry container settings must use a kind that older pages cannot decode, so an
  older open page cannot approve options it never displayed.
- #3179, #3180 and #3181: three findings about old terminals (the tmux-based ones) discarded. The maintainer chose to
  discard them because support for old terminals is about to be removed, and triage judged none of them very serious.

#### What landed since the stack was based, and whether it interacted

The executor read everything that reached main up to and including the Tensorlake state-path refusal (#3168, #3170),
which makes Tensorlake connections refuse a helm state folder whose path contains `%`, `"` or `\`. After that, main
gained the previous landing round's two plans: agent-settings-compare-replace (#3173), which stops Farhelm's credential
delivery to a Tensorlake host from overwriting an agent settings edit saved meanwhile, and maintainer-tool-safety
(#3169, #3172), safety fixes in two maintainer scripts. Nothing else reached main before this landing, and this round
landed no other plan.

The landing monitor and, independently, a fresh-context review read these against this plan and found no interaction.
Credential delivery's record of what it delivered is part of the host's saved entry, so host removal now deletes that
record durably too, and a delivery that finishes after the host was removed changes nothing. The maintainer scripts
share nothing with this plan.

#### Things you should know

- Every way of removing a host goes through the one changed removal step, so removing a Tensorlake host, and rolling
  back a failed add, get the durability too, which matches the changelog entry's "Removing a host".
- Any change to the Custom words now counts as a new launch choice, including one that only adds or removes spaces. Two
  effects follow. During a resume it cancels the pending Launch press, which is the fix. And for a fresh GitHub checkout
  launch on any host, it re-requests the checkout preview, so a Launch press right after an edit waits until that
  preview returns. Edits to the wrapper and invocation text already worked this way, so this is consistent rather than
  new; the changelog entry does not mention it, and the landing monitor sees no need for a follow-up. Picking a recent
  launch or a template, or changing the harness or permission mode, can also rewrite the Custom words, but each of those
  already changed another launch choice and so already cancelled a pending press before this plan. The review found no
  way for the Custom words to change without a user action.
- The launcher feedback item that #3174 narrowed lost its top-level "Reviewed commit:" line and its "## Details"
  heading, which the feedback file template asks for. The reviewed revisions are still in the file, one section down, so
  the landing monitor left it as is.
- The specification now accepts the right-edge sign placement "under the tmux parity rule", but the vendored terminal
  emulator's notes, which list where Farhelm's terminal deliberately differs from Ghostty's, do not mention it. Nobody
  checked whether Ghostty places the sign differently. The plan deliberately left the vendored code and its notes alone.
  A possible follow-up is to check Ghostty and add the entry if it differs.

#### Checks

These ran once on the whole stack, rebased onto main `88a4c942`, which is the tree that landed (no fixes were made at
landing):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the UI's web-target clippy
  (`cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`),
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), `cargo build` and the UI's release web build
  (`dx build --package farhelm-ui --platform web --release`): all passed.
- Through the test recorder (`python3 scripts/record-test-run.py --kind development ...`), with pinned nextest 0.9.148,
  four parallel test slots and no retries:
  - the helm crate's tests, `cargo nextest run -p farhelm-helm`, with the pinned tmux 3.7c required, run
    `59c1ac64-6a57-40d3-810b-b97706368342`: all 1569 passed;
  - the UI crate's tests in its desktop build, `cargo nextest run -p farhelm-ui --features desktop`, with no tmux
    (`--tmux none`; these tests use headless component state), run `eee1198a-d5bb-40aa-ab7e-b19a40b067b5`: all 1097
    passed.
- The browser specs for the launcher's permission choices, Enter-to-launch, restart-with, resumes and GitHub checkouts
  (`custom-permissions`, `guarded-permissions`, `yolo-guard`, `launcher-enter`, `restart-with`, `tensorlake-lifecycle`,
  `github-checkouts`), on Chromium and WebKit (`--runner playwright`, `npx playwright test` with those seven files, the
  pinned tmux 3.7c required, three workers, no retries), run `026afc7b-4cd1-4761-aa5c-3155c7287dfa`: 325 of 328 passed.
- The three browser failures were all WebKit `github-checkouts.spec.ts` tests that hit the 60-second test limit:
  "replacement preserves borrowers and archives only the released checkout", "clone defaults to managed checkouts and
  searches only untouched names", and "trash count and cue follow the final reference, including another client". Their
  Chromium copies passed. All three ran in the same browser stack on a busy shared machine of 18 CPUs: the 15-minute
  load average was still about 14 when the run ended. These tests never edit the Custom words, so nothing #3174 changed
  varies while they run. A narrow re-run of just those three tests on both engines on the same tree, run
  `361b7691-30f1-4791-aadd-fe5573158d5b` (same recorder settings, `--grep` on the three names), passed all six in under
  two minutes, at a load average of about 5.
- FLAKES.md has earlier timeouts in this file. The 2026-10-08 entry records timeouts at the 60-second limit that
  included the same "replacement" and "clone defaults" tests, on Chromium there, with its cause unknown. The 2026-11-05
  entry gave a different test in the file a longer time limit and removed the file's Deflake entry, saying that did not
  establish every earlier timeout had the same cause. No open Deflake entry covers these three tests now. Following
  `AGENTS.md`, "Sharing the machine with other agents", a single failure under load is treated as an environment event,
  so these were not logged as a regression or a new flake. If they fail again, they should get a FLAKES.md entry.
- Reused: nothing from the executor's runs.
- Not run: the rest of the browser suite and the other Rust crates, the UI's JavaScript tests, the docs website build, a
  real Tensorlake sandbox and the native Mac app. The code changes are confined to the helm's host-removal write and the
  launcher's check for whether the launch choices changed, which the runs above cover; the other six PRs change only
  documentation and bookkeeping.

Nothing in the executor's report became untrue at landing, beyond the PRs' draft state and bases: all eight are merged,
each onto main in stack order.
