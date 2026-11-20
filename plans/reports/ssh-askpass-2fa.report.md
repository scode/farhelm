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
