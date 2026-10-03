### What this was about

The desktop app runs its own helm inside it, and its window and its native side talk to that helm over the same loopback
connection a browser uses. Until now they also signed in the way a browser does: they traded the helm's sign-in token
for two remembered credentials, one for the window and one for the app's native side. So `farhelm helm token rotate`, or
enough browser sign-ins to push the oldest remembered credentials past the helm's limit of 64, signed the desktop app
out, and it quietly signed itself back in. Two review findings showed that hidden recovery going wrong in ways users
could see: the window could open with terminals and a session list that could not connect (a failed write of the new
credential into the window's storage was ignored), and a failed recovery replaced the whole app with an error page, so
an action already running, such as a delete, never showed its result. A third finding was the browser version of that
last problem: a browser's sign-in prompt replaces the page, and a pending action's result is lost there too (the helm
still does the work).

In triage you decided that the desktop app must never involve a credential the user ever has to deal with, so its own
credentials should be exempt from token rotation and the 64-credential limit, rather than its recovery being hardened
step by step. For the browser you decided that losing a pending action's report across a sign-in is acceptable: the
browser is best effort for rare problems that lose a report but not work. You also decided the desktop error page with
its Retry button stays as a last resort for something genuinely broken.

The plan carried out those decisions in three PRs:

- **The desktop app's credentials are exempt.** The helm inside the desktop app now creates the app's two credentials in
  memory when it starts and hands them over directly. They are never remembered in the helm's database, so rotation and
  the 64-credential limit cannot touch them, and rotation no longer closes the desktop window's open terminals or live
  session list. No browser can obtain one, so rotation still signs out every browser. They end when the app quits, so
  each launch gets a fresh pair and the app no longer keeps credentials in its state file. With nothing to revoke, the
  desktop's hidden sign-in-again machinery is gone. The window opens only after it has stored and proved its credential;
  a storage write that fails now lands on the error page instead of opening a window that cannot connect.
- **The desktop outcome-loss finding is closed.** Once the window is open, nothing replaces it any more: the error page
  can only appear before the app is first shown, or from that page's own Retry button, so no pending desktop action can
  be cut off.
- **The spec carves the browser out** of "an action is never lost silently", keeping that promise for the desktop app.

### Things you should know

- **A trade you have not been asked about directly: rotation can no longer cut off a stolen desktop credential.** The
  window's credential still sits in the window's storage, where a script that got into the window (through some
  injection bug nobody has found) could read it, exactly as in a browser. Before, `farhelm helm token rotate` was the
  way to cancel such a stolen credential. Now nothing but quitting the desktop app does. This follows directly from your
  decision that the desktop app's credentials are exempt from rotation, and in one respect it is narrower than before:
  the old desktop credentials were remembered across launches and lived until a rotation, while these end when the app
  quits. The security notes (`docs/security.md`) now say this plainly. SPEC.md's "Client hardening" holds the native app
  to a higher bar, which is why it is flagged here rather than left in the docs.
- **Upgrading.** The first launch of the new build rewrites the desktop app's state file without the credentials an
  older build kept there. The credentials those older builds stored in the helm's database stay valid until the next
  rotation or until the 64-credential limit pushes them out, like any old browser credential; nothing cleans them up
  sooner. Going back to an older build is fine: it finds no saved credentials and signs in the old way.
- **The desktop smoke test changed contract** (it runs as part of the release gate). It used to check that the desktop
  app's credentials survive a restart and that the app re-signs-in after rotation. It now checks that the desktop app
  stores no credentials and the helm remembers none for it, and that after rotation a browser credential is refused
  while the desktop window does not sign in again, its terminal stays on the same connection, and its native requests
  still succeed. The smoke test signs in as an ordinary browser for its own requests. One smoke-only log line was added
  to make that last check possible.
- **A 401 in the desktop app is now just an error.** Since its credentials cannot be revoked, there is nothing to
  recover with; if it ever happens, something is broken, and the action shows an error rather than quietly retrying.
- **Review changes beyond the findings themselves.** A failure to record a small readiness counter that only the smoke
  test reads no longer puts the window on the error page (it is logged instead), and the state file is rewritten once
  before the window opens, so a broken state file still fails before any window appears, as it did before. The browser
  sign-in route still allows the desktop window's origin to call it, though the window no longer does; that was left
  alone as out of scope.

### Open questions and possible follow-ups

- **Is the stolen-credential trade above acceptable as is?** Options: accept it (my recommendation: it is the direct
  consequence of the decision, and narrower than what it replaced); or ask for a follow-up, for example having
  `farhelm helm token rotate` print a note that a running desktop app keeps its own credentials until it is quit.
- Possible cleanup follow-up, not needed for this fix: stop allowing the desktop window's origin to call the browser
  sign-in route, which nothing uses any more.

### The PRs

- #1487 `fix(desktop): exempt the app's own credentials from rotation and the cap` — the code, tests, smoke, specs and
  security notes, plus a changelog entry: https://github.com/scode/farhelm/pull/1487/changes
- #1488 `docs: close the desktop sign-in outcome-loss finding` — records the check and closes the finding:
  https://github.com/scode/farhelm/pull/1488/changes
- #1489 `docs: let a browser sign-in lose a pending action's report` — the browser carve-out in SPEC.md:
  https://github.com/scode/farhelm/pull/1489/changes

### Checks run, reused and skipped

- Run on the final tree of #1487: `cargo fmt --all -- --check`;
  `cargo clippy -p farhelm-helm -p farhelm-ui
  --all-targets -- -D warnings`;
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`; `cargo
  check -p farhelm-desktop`; the
  desktop UI tests for the sign-in, desktop and API modules through the recorder
  (`cargo
  nextest run -p farhelm-ui --features desktop`, selection 'desktop ui auth/desktop/api modules', run
  d764cafb, 96 passed); the JS harness (`node --test` in `crates/farhelm-ui/js-tests`, all passed, including the
  rewritten window sign-in tests); the desktop smoke (`scripts/desktop-smoke.sh` through the recorder with the pinned
  tmux, run a58b8775, passed); `scripts/check-test-sleeps.py` (no unannotated delays); `dprint check` on every changed
  Markdown file; `python3 releasing/check-changelog.py format`.
- Reused: the helm's sign-in tests (`cargo nextest run -p farhelm-helm`, selection 'helm auth module', run 1a76d076, 41
  passed, including the four new ones), run before the review. Since then the helm's sign-in code changed only in
  comments and formatting.
- Skipped: the full Rust test suite and the browser end-to-end tests. The browser's sign-in flow did not change, and the
  focused tests, the smoke and the type checks cover what did. #1488 and #1489 change only Markdown, so they got
  `dprint check` and nothing else.

### The review gate's outcome

A fresh-context Opus 5.5 review of #1487 (the plan asked for high effort; the sub-agent mechanism could not set effort,
so it ran at the executing session's own) found no blocking defect. It confirmed that no browser route can obtain a
desktop credential, that every former desktop recovery path now reports an error instead of creating a revocable
credential, that every part of the window still gets a credential, and that the new smoke checks would have failed under
the old design. All seven findings were acted on: (1) the stolen-credential trade is now stated in the security notes
and above; (2) the readiness-counter write is no longer fatal and the state file is rewritten before the window opens;
(3) the smoke now proves the app's native requests still work after rotation; (4) a dozen stale comments and spec
passages that described the removed recovery were corrected; (5) a spec paragraph about browsers that had drifted into
the desktop paragraph was moved back; (6) of two leftovers the reviewer flagged as optional, a now-pointless timing
branch was removed, while a guard rule in the window's sign-in state (never take the app off screen once shown), which
nothing currently exercises, was kept deliberately, which the reviewer agreed was defensible; and (7) one test's
description was narrowed to what it can actually catch.
