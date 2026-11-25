# Host provisioning action fixes

## What this was about

A confirmed Uninstall on a Tailscale host used to resolve the destination again for individual removal commands. If the
address or keys behind the name changed, those commands could reach a different machine from the one Farhelm checked.
Uninstall now verifies the prepared peer at confirmation and keeps its address, keys and connection policy for the whole
run. It asks the user to reconnect or plan again when the session connection or Tailscale peer has changed.

Answering a shared SSH sign-in question could start an automatic update before the Update or Uninstall the user had
requested retried, leaving the manual action to wait for the host to become available. The agreed behavior gives the
explicit action first use of the answer. An automatic update can try again after the manual run settles or after 30
seconds without a manual run. Turning Tailscale on or off dismisses an automatic-only question and gives its connection
check a bounded time to finish. A question already owned by a manual action remains available to answer.

## Things you should know

The installer-identity check alone could accept an older Sessions connection and a fresh Uninstall probe if neither peer
reported an identity. Both confirmation session reads now check that the active Sessions connection opened with the same
Tailscale peer and policy as the removal commands. An unrelated change to a local SSH socket directory does not count as
a peer change.

If a manual request times out before Farhelm returns its sign-in prompt, the request may need to be retried. Answering
the shared question can still defer automatic Update for up to 30 seconds. That delay closes the gap between finding the
question and delivering the reply. When the user turns Tailscale on or off, Farhelm keeps the question open only if the
manual action already owns it. A cancelled or failed shared check suppresses further automatic prompts on that
connection.

## Open questions and possible follow-ups

Both planned changes fit the approved scope.

## Pull requests

- Uninstall keeps its confirmed Tailscale peer for the session checks and removal steps.
  [#4996](https://github.com/scode/farhelm/pull/4996/changes)
- Explicit host actions get priority after shared SSH sign-in, and mode edits retire automatic-only checks.
  [#5002](https://github.com/scode/farhelm/pull/5002/changes)

## Checks run, reused, and skipped

- The combined run on the rebased stack executed 91 selected cases successfully and recorded one runtime skip. It
  covered peer pinning and refusal, shared answer handoff in both join orders, retry after expiry or manual completion,
  failure across opt-out, and Tailscale route editing. Recorded run `784ef40d-b140-4732-9201-17095452751e`.
- Reproduce the selection with the pinned nextest through the test recorder. The repository's `.config/nextest.toml`
  sets four test slots and zero retries.

  ```sh
  python3 scripts/record-test-run.py --runner nextest --kind development \
    --selection "Rebased host provisioning stack and SSH paths" \
    --concurrency "4 nextest slots; retries 0" -- cargo nextest run -p farhelm-helm --lib \
    -E 'test(/automatic_sign_in::|automatic_update|automatic_admission|obsolete_route_refusal|tailscale_toggle|uninstall|ssh_sign_in::|ssh::tailscale::|access::tests::|transport::tests::|remote_routes_keep_existing_plan_json/)'
  ```

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py` with the repository's isolated interpreter, and
  `git diff --check 607a91e8 0331d8d6` passed. Both PR diffs stayed intact through rebase. The combined run supplies the
  final runtime evidence. Earlier isolated peer-fixture development evidence remains in run
  `c8e99837-7507-41eb-a093-2c68b7b85fb3`.
- The live terminal-host test self-skipped because the workspace Farhelm binary was absent. The selected tests cover
  peer authority, shared question ownership, and route changes through isolated provisioning and SSH fixtures, so the
  wider workspace and UI suites were skipped. Real-host removal and a live terminal host stopping during removal remain
  unverified in this run.

## Review gate

Independent source review verified the session connection uses the same peer as the confirmed plan. The sign-in review
also checked Ready arriving before activation, failure after opt-out, and a queued reader returning after a mode change.
The final follow-ups found no remaining issues.

### Landing

#### What landed with it

Only this plan landed this round, through its two PRs and the landing fix
[#5009](https://github.com/scode/farhelm/pull/5009/changes). Since the plan's PRs were based, main gained only the
launcher restyle of the session switcher, template list and Rename (five PRs), which changes the app's drawing and
nothing in the helm. The rebase applied without conflicts, and nothing in it interacts with this plan: the app's host
Update, Uninstall and Tailscale host controls show the helm's refusals as they come and match on none of their text, so
the one refusal this plan reworks (a changed connection setting) reaches them unchanged, and neither side bumps the
protocol or a database version.

#### Fixes made while landing

The should-fix items below come from the landing's own independent review, not the executor's review gate.

- **The helm did not build on its own.** The Tailscale switch now uses a helper from the `futures-util` library that the
  helm had switched on only for its tests. Builds that include the desktop app still compiled, but two builds of the
  helm without it failed: the Podman test payload (so the test worker's suite stopped before its first test) and
  clippy's check of the library. Fixed inside [#5002](https://github.com/scode/farhelm/pull/5002/changes) by switching
  that feature on for the helm itself; it changes nothing else.
- **Turning Tailscale host on or off could freeze the app for 20 seconds (first should-fix).** The plan made the switch
  wait for the host's connection instead of answering busy at once, so it can dismiss a sign-in question that only an
  automatic update opened. It waited up to 20 seconds. The app sends its commands to the helm one at a time, so
  everything after the switch waited too. That included the user's answer when the question holding the connection
  belonged to their own Update or Uninstall, so in that case the switch could never succeed and ended as busy after 20
  frozen seconds. And a connection freed late in those 20 seconds, plus the reconnect that follows, could pass the app's
  30-second limit for a reply, after which the app resets the whole window. #5009 shortens the wait to five seconds,
  which covers dismissing an automatic-only question, since that frees the connection at once; five seconds plus the
  reconnect's own limit of 15 now come to 20, well under the app's 30, which leaves the switch's remaining short steps
  room before a reset. The user-owned case still answers busy, after at most five seconds of waiting; I recommend no
  further change, since the app already refuses the switch while it is itself waiting on that host's sign-in, which
  covers the usual way into that case.
- **The Uninstall peer test could not catch the run using planning's connection details (second should-fix).** Uninstall
  keeps the Tailscale peer's address, keys and connection policy from confirmation for the whole run. The test gave
  planning and confirmation identical details, so it passed even if the removal ran with planning's, which can name an
  SSH connection the confirmation did not sign in. #5009 gives confirmation the same peer over a different SSH
  connection and checks that every removal step uses it; with the line that makes the run use confirmation's details
  removed, the test now fails (test-worker recorder run `b845f47f-127f-40cb-95c8-92940df5bf1e`). The code itself was
  right.
- **SPEC.md left out what users see from the sign-in change (third should-fix).** #5009 adds it: the user's own Update
  or Uninstall gets a shared answer first and the automatic update waits for it or 30 seconds, and turning Tailscale
  host on or off dismisses an automatic-only question. It also clarifies the plan's Uninstall sentence in SPEC.md and
  one SPEC_impl.md sentence, adds code comments the review asked for, corrects one test's description, and rewords both
  changelog drafts to name the **Tailscale host** setting and the 30-second wait.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The plan's PRs were
rebased onto main as it stood at the start of the landing (`9bf0270b`), which applied without conflicts; every file's
change was exactly what the executor pushed until the two landing fixes.

- The first workspace run on the rebased stack started no test: its Podman test payload build failed to compile the helm
  (recorder run `68a6e224-dfd0-44f7-bd16-a2153b26b0b6`), and `cargo clippy --all-targets -- -D warnings` failed the same
  way. That is the build problem fixed inside #5002.
- After that fix, the workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test
  worker (recorder run `aa446201-7fe8-4c41-a37e-16dcd2097a84`): all 8722 passed, with no runtime `SKIPPED` lines.
- After that fix, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings` and
  `cargo clippy -p farhelm-helm --lib -- -D warnings`, on the test worker: passed.
- After [#5009](https://github.com/scode/farhelm/pull/5009/changes), the helm's whole test suite,
  `cargo nextest run -p farhelm-helm`, on the test worker (recorder run `f2b228c6-888a-4f47-9adc-a8f33eccc19a`): all
  1930 passed. I did not count this run's runtime `SKIPPED` lines; the workspace run above, which covered the same tests
  before #5009 and counted them, had none. Also `cargo clippy -p farhelm-helm --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-helm --lib -- -D warnings` there: passed.
  [#5009](https://github.com/scode/farhelm/pull/5009/changes) changes only the helm, its tests, the specs and the
  changelog drafts.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `3d9a06c8-abb1-44ad-8b1a-d9625e7680dd`), on the rebased stack before the fixes: 50 steps passed, one skipped by design
  on Linux (paste), including the host Update and Uninstall steps, which go through the helm's provisioning service. Its
  app build compiled before the build fix because that build includes the desktop app. The fixes since change only a
  build setting, the Tailscale switch's wait, which no smoke step uses, a test and documents, so this run still applies.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive, which main has had since an earlier triage record (#4978)
  landed (the executor's report says `dprint check` passed, which holds for the plan's own files). No PR fixes them yet;
  a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused: the smoke run above, for the reason given. The executor's runs were focused selections on an older base, and
everything they covered ran again above.

Skipped: the desktop app's own tests and the screenshot goldens, since nothing in the app or its drawing changed; a Mac
run, since nothing changed is Mac-specific; real Tailscale hosts, which no automated check reaches (the report says the
same); and CI, which runs only on demand.

#### What changed from the report

- The report says a question a manual action owns stays available to answer. It still does, but turning Tailscale host
  on or off while such a question holds the connection now answers busy after five seconds rather than 20.

#### Things you should know that the report does not say

- **One narrow ordering is still possible.** If a manual Update starts a fresh sign-in question before the automatic
  update has noticed it, and the user answers before the automatic update looks, the automatic update can still start
  first. SPEC_impl.md scopes the guarantee to a manual action that finds an automatic question already waiting, so this
  matches the specs. No follow-up recommended unless you want that case closed too.
- **Smaller points the review noticed.** An upload over a Tailscale connection that Uninstall has fixed to one peer
  would look the peer up again, which is harmless today because Uninstall uploads nothing; on a disconnected host with
  Tailscale not running, Uninstall reports a raw failure instead of the usual "not connected" refusal. Neither needs a
  follow-up on its own.
