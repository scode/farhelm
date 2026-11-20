# SSH sign-in prompts

## Since the last review

This round amended #4299, #4300, #4303 and #4305. The keepalives in #4296 and the visual attention cue in #4302 carry
the same changes as the first delivery.

Setup now lets you finish signing in to the SSH connection used for that host's sessions. Previously the last step of
Add Host could restart that connection while you were typing a code, or report a timeout before the question expired.
Setup waits while the question is open, up to the question's own limit. If you cancel or the question expires, setup
reports **Sign-in needed**. Closing Add Host during its first check ends that check even if the host has yet to ask a
question. When you resume an Uninstall that previously failed after removing its service, it continues after you answer
the SSH question if the same Hosts entry remains and its supervisor is still down.
[#4303](https://github.com/scode/farhelm/pull/4303/changes)

A host that cannot be reached and never asks a question now shows the ordinary connection error, both when checking a
destination and when connecting for sessions. The tests that failed in the first landing round now read each group of
desktop updates by name and build an accurate older database.
[#4299](https://github.com/scode/farhelm/pull/4299/changes) [#4300](https://github.com/scode/farhelm/pull/4300/changes)
[#4303](https://github.com/scode/farhelm/pull/4303/changes)

The startup focus exception now applies only to the connections Farhelm starts when it opens, and only before your first
key or click. If a host first becomes reachable later, or asks again after a dropped connection, its question waits in
the sidebar. Actions you start can still open their own questions. This follows the startup decision that the maintainer
confirmed after the landing review. [#4300](https://github.com/scode/farhelm/pull/4300/changes)

The Session list docs now describe the new chime and when it can sound on startup.
[#4305](https://github.com/scode/farhelm/pull/4305/changes)

## What this was about

Hosts that ask for a password, code or other interactive SSH answer could stay disconnected until you ran SSH by hand.
Farhelm now shows questions that OpenSSH passes to askpass. Add Host shows the setup question inline. A registered host
shows **Answer** above the session list and an amber key on the navigation rail. When one of the shared SSH connections
goes silent, keepalives let it detect the loss in about a minute.

If sessions sign-in fails after asking a question, including cancellation or a two-minute expiry, Farhelm ends that
sign-in and shows **Sign-in needed** until you choose **Sign in**. If the host never asks a question, it stays on the
ordinary retry schedule. Farhelm passes answers to SSH in memory.

## Things you should know

- Add Host can ask once for setup and once for sessions. The setup connection can ask again after being left idle at the
  confirmation. Cancel keeps the destination and alias.
- **SSH sign-in** in **Settings → Notifications** controls the chime and defaults to on. The amber cue moves to the row
  or flashes in place with reduced motion. If the row vanishes, one frame can use its old position.
- A compatible live shared SSH connection started by hand is still reused. The sign-in flow needs a usable SSH control
  socket path. A path that is too long or that SSH would expand can leave only ordinary key authentication available.
  Setup needs one more byte, so a path near the length limit can work for sessions and still fail during setup.
- Automatic update discovery continues to use batch checks. Manual Update can ask when authentication is needed. An
  automatic update that has already started can also ask if it needs a new setup connection.
- **SSH needs 2FA?** has been removed. The website's **Add a remote host**, **Send feedback** and **Session list** pages
  describe the current behavior.

## Open questions and possible follow-ups

Native macOS rendering and a live Duo or security-key exchange through the desktop remain for release verification.
Linux coverage uses a fixture Duo-style menu and touch notice. The live OpenSSH checks cover an unknown host-key
question, a protected key's passphrase, and handoff to the shared connection. An actual touch notice depends on what the
SSH client exposes to askpass.

If someone requests a reconnect while setup is finishing and automatic discovery also requests a retry before the
connection handles either request, the reconnect goes ahead. In that ordering, setup can report that it could not attach
if the supervisor is slow to come up. Sessions can reconnect on a later background retry. This remains a follow-up.

## Pull requests

- Keepalives for shared SSH connections. [#4296](https://github.com/scode/farhelm/pull/4296/changes)
- Relay SSH questions for sessions. [#4299](https://github.com/scode/farhelm/pull/4299/changes)
- Desktop prompt, focus, status and chime. [#4300](https://github.com/scode/farhelm/pull/4300/changes)
- Sidebar attention cue and reduced motion. [#4302](https://github.com/scode/farhelm/pull/4302/changes)
- Sign-in while adding, setting up, updating and uninstalling hosts.
  [#4303](https://github.com/scode/farhelm/pull/4303/changes)
- Remove manual SSH help and update the docs. [#4305](https://github.com/scode/farhelm/pull/4305/changes)

## Checks

- The full `cargo nextest run --workspace --exclude farhelm-desktop` run at `a4b05323` executed 8,145 tests. It passed
  8,143, including all five tests from the first landing review. Nextest reported 74 skipped and the retained console
  contained no runtime skip messages. Two tests failed (218411fa-0969-49ba-bf20-b5b22d75511f). The Add Host test's
  assertion required a question while testing the period before the question arrives. Its corrected assertion checks the
  pending attempt and absence of a question before dismissing the dialog. The real Podman environment test could not
  start the sandbox's user manager within 30 seconds.
- The Podman test ran to completion with real rootless Podman on clean main at `c3569dc5`
  (7505ade3-a4cc-48da-ba40-37a114945a71) and on the final stack at `4212edb9` (e6a85b77-734a-4592-b14a-a4e301e0a4ab).
  The original startup failure did not recur in either run. The selection was
  `cargo nextest run -p farhelm --test e2e -E 'test(=podman_environments::real_local_environment_lifecycle_and_owned_image_cleanup)'`.
- On `4212edb9`, `cargo nextest run -p farhelm-helm -p farhelm-gui` passed all 4,983 selected tests with no runtime skip
  messages, including the five landing failures and the corrected Add Host test. Nextest listed four skipped
  (21de2e6b-9d21-43af-9922-d6a6eaf1aa6d).
- The changes after the workspace run are confined to the helm's connection actor and the Add Host test. The workspace
  pass results cover the rest of the stack. Rust formatting, the diff whitespace check and the test-delay checker
  passed.
- On `4212edb9`, `python3 scripts/check-gui-goldens.py` matched all 298 Linux reference screenshots
  (ccaa9c2a-6842-4000-9b23-2f73577f2ba4).
  `cargo clippy --locked -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings` also passed.
- Reused the private OpenSSH 9.6 checks from the first round for the client arguments, relay and authentication handoff.
  The keepalive probe used the in-progress keepalive changes based on `ee1591ab`. Both shared connections closed after
  59.7 seconds without traffic (`python3 -B keepalive-probe.py`, run 033bec48-c5d5-4a23-8311-20dc8d9e592f). An unknown
  host-key question and a protected key's passphrase reached the private helper and an SSH control-socket check then
  passed with the in-progress sign-in helper based on `9363cb6d` (`python3 -B askpass-openssh-probe.py farhelm`, run
  18954bc6-6891-45e8-bdec-9bebe688d191). The same check observed one key passphrase question through ProxyJump during a
  deliberately failed connection. The follow-up changes when Farhelm preserves or restarts a dial and how it reports a
  deadline. The probe's OpenSSH arguments, helper exchange and shared-connection handoff are unchanged.
- `bun run build` built 38 docs pages and checked all internal links. The exact build commit was unavailable. The
  working log records the build on the final docs content, which matches `29e3e0f4` and `4212edb9`.
- The landing monitor checks the desktop shell suite. The release gates run during release verification. The workspace
  reuse and targeted reruns cover the files changed after the workspace run.

## Review

Every draft passed the required independent source and wording review in the first round. The unchanged attention cue
retains its original Opus design review. The four amended drafts received independent source review in this round. Setup
review confirmed that the existing retry channel can keep an open SSH question while setup waits for the sessions
connection.

### Landing

#### What landed with it

It landed alone in its round, on a main that had gained only plan bookkeeping and TODO.md edits since its base, so
nothing else interacted with it. This was its second landing attempt: the first stopped on five failing tests, two bugs
(setup killing or outwaiting the sessions question, and unreachable hosts reporting a sign-in timeout), the startup
focus exception that never ended, and smaller items. An independent reviewer and I checked this delivery against that
list. Every item is fixed, and the two bugs and the startup change each have a test that reaches the broken state.

You asked that this delivery be fixed and landed by the landing monitor rather than sent back again, so the fixes below
went in as a seventh PR, [#4308](https://github.com/scode/farhelm/pull/4308/changes).

#### Fixes made while landing

- **Add host or Update could still fail after a good install when a Retry landed at the same moment.** When setup's last
  step and a user's Retry or Sign in on the same host reached the host's connection before it read either, the
  connection dropped setup's request for a fresh retry window. A supervisor that was still starting then made the run
  fail with "waiting for the provisioned supervisor: timed out". This was the race the report listed as a follow-up,
  plus a second ordering the report did not name. Setup's window now survives whichever request came last, with a test
  of each ordering.
- **The desktop test of a new host's two sign-ins used an ordering that cannot happen.** It finished the run before the
  sessions question arrived; the run now stays going while that question opens, as in a real Add.
- **Wording.** SPEC.md's sentence on which later questions may take focus is split so each exception reads on its own;
  SPEC_impl.md's "sign-in needed" now also names an edit of the host's connection settings as a restart; one more test
  reads the hosts collection by name rather than position; and the changelog fragment notes the fix for curation.

I also tried ending the startup exception when the window reconnects to a helm, against a helm restarting under an
untouched window. That cannot happen: the desktop app's helm runs inside the app and the app exits when it ends. I took
the change back out before landing.

#### Checks

The plan touches the helm, the protocol, the CLI, the desktop app, the GPUI GUI, docs and the website, so the landing
ran the whole workspace suite, the desktop app's tests, the end-to-end smoke and the four clippy runs on the
maintainer's own test machine (an arm64 Linux VM that builds and tests the tree itself), and the goldens, the website
build and the small checks locally on x86_64.

On the plan as delivered, rebased on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`7b18720d-8acc-44a8-aa02-34a16fe0fbc1`): all 8145 passed, with no runtime skips.
- `cargo nextest run -p farhelm-desktop`, on the test machine (`7d811181-8b0f-463a-a3d8-4ff0a1d6c227`): all 176 passed.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine: 49 of 50 steps passed, the
  clipboard paste step skipped as always on Linux (`f21ef797-5453-4997-9d0b-027b36f14be2`).
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  clippy runs, on the test machine: passed.
- `python3 scripts/check-gui-goldens.py`, locally (`fa0a8c20-e022-478b-a167-f4ffa76a92f2`): passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-gui-e2e-smoke.py`, `python3 -B scripts/test-gui-screenshot-batch.py` and the website build
  (`bun run build`), locally: passed.

On the fixed tree:

- `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-helm) | package(farhelm-gui)'`, on the
  test machine (`b62158f3-65b6-4f02-b3d2-b38b29cfccfc`): all 4985 passed, with no runtime skips. Its tree still held the
  startup change taken out afterwards; that part of the desktop app is back to the delivered code the workspace run
  covered.
- The sign-in tests and the connection's request-merging tests on the final tree, locally
  (`603dcaa8-cc58-4fbd-9838-62feb2055d12`): all 11 passed.
- The two default-member clippy runs on the final tree, on the test machine: passed. `cargo fmt --all -- --check`,
  `dprint check` on the specs and the changelog format check, locally: passed.

The smoke, goldens and desktop app tests were reused for the fixed tree: the fixes change the helm's connection actor,
tests and documents, nothing the app draws or its startup.

Skipped: anything on a Mac, and a live OpenSSH or Duo exchange through the desktop app, which stay with release
verification as the report says.

#### Things you should know that the report does not say

- **A new 2FA host whose sessions question expires during a long install now ends its Add run "Sign-in needed"** after
  the install itself succeeded. That follows the decision that setup must report sign-in needed rather than a timeout;
  **Sign in** on the host then connects it.
- **Re-adding a destination whose host is at "Sign-in needed" no longer restarts its sign-in by itself.** The host waits
  for **Sign in**, which matches SPEC.md's rule that a stopped sign-in waits for an explicit retry.
- **Smaller follow-ups the review found and the landing left alone:** registration and Update pick between two retry
  paths by whether setup sign-in is configured, where one path would do; closing Add host cancels its check even when
  the dialog then stays open (not shown to be reachable); a question from a check whose reply never reached the window
  can still chime for up to two minutes; and the uninstall resume fix has no test.
