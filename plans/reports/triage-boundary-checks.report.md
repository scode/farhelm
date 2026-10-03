### What this was about

Five review findings about checks at Farhelm's boundaries, where the helm or the supervisor accepts input it did not
produce. In triage (2026-10-02) you decided: "yes - when cheap. we don't expend tons of complexity for defense in depth
at every level, but reasonable straight-forward defensive checks are encouraged." Each fix below was small, and none
changes what someone using Farhelm normally sees.

1. **Session ids from supervisors.** The helm checked session ids in a create's reply (no empty id, no control
   characters) but not in session lists, and neither path refused an id of `.` or `..`. The browser puts session ids
   into request paths, and URL parsing treats an encoded `..` as "go up a level", so a Stop on such a session would be
   sent to a different helm route. No current route makes that harmful. The helm now applies one rule to every session
   id a supervisor sends. A session list containing a bad id is refused whole and the previous list kept, as already
   happened for oversized or duplicate ids.
2. **Profile edits with a misspelled key.** The resume command in a saved profile is optional, so an edit through the
   API that misspelled its key was accepted and silently erased the stored resume command. The API now refuses unknown
   keys when creating or editing a profile, and names the key.
3. **The provisioning lock map.** The helm keeps a per-host lock so installing or updating a host waits for, or blocks,
   editing or removing it. Edit requests took the lock before checking the host exists, and entries were never removed,
   so a signed-in client could grow the helm's memory by naming made-up host ids. Entries now go away once nobody holds
   or waits on them.
4. **Restart with.** "Restart with" saves a new agent command and resume command for the session, but the supervisor
   saved them without the checks it applies at create. At startup the supervisor refuses to load its sessions if any
   saved session fails those checks, so one bad request could have left it unable to load any session after its next
   restart. The helm never sends such a request. The supervisor now refuses it, as an invalid request, before anything
   is stopped.
5. **Shortened screen-reader labels.** Labels for the host and session menus show invisible characters as markers such
   as `<U+202E>` and are cut to 64 characters without splitting a marker. The cut assumed markers are always eight
   characters, but some are nine or ten (`<U+E0041>`, `<U+10FFFF>`), so a cut could leave a broken fragment. It now
   handles every marker length.

### Things you should know

- **A browser-side gap is documented, not fixed.** Finding 1's helm check covers session ids only. Tab ids also go into
  request paths, and a supervisor-supplied tab id of `..` would still resolve to a different path. No route answers that
  today, and the plan scoped the fix to session ids. The code comments now say this plainly instead of claiming the
  browser's encoding prevents it. Checking tab ids too would be a separate decision.
- **A slightly different answer for some malformed restart-with requests.** Because item 4's checks now run first, a
  malformed restart-with aimed at a missing or unsuitable session answers "invalid request" instead of "not found" or
  "conflict". That matches how create orders its checks.
- **A pre-existing test failure on main, unrelated to this plan.** While running the supervisor and end-to-end restart
  tests, the end-to-end test `lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart` (fresh GitHub
  checkouts) failed every time. It fails identically on unmodified main at the commit this stack is based on. It expects
  a refusal naming changed checkout settings, and instead gets the refusal for a host connection that changed across the
  helm restart. This plan did not investigate or change it.
- All five changelog fragments are `kind: none`, since only a broken or hostile peer, or a third-party API client, could
  reach these paths.

### Open questions and possible follow-ups

- Whether tab ids should get the same `.`/`..` check as session ids (see above). The recommendation is to leave it until
  a route makes it reachable, since nothing answers it today.
- The failing fresh-checkout end-to-end test above probably deserves its own look.

### The PRs

One linear stack, each based on the one before:

- #1491: fix: check supervisor session ids the same way on list and create.
- #1492: fix: refuse unknown keys when saving a profile.
- #1494: fix: drop a host's provisioning lock once nobody uses it.
- #1495: fix: apply create's checks to a restart-with launch.
- #1497: fix: keep a shortened label from cutting an escape marker in half.

### Checks

Run now:

- `cargo fmt --all -- --check` and `cargo clippy -D warnings` (all targets) on the helm, UI and supervisor crates, and
  on the `farhelm` crate for the new end-to-end test: clean.
- Recorded nextest runs:
  - PR 1: a6c0c9f7 (94/94, helm id ingress and refresh tests plus the browser path-segment test) and e218b527 (after the
    review fixes).
  - PR 2: 99b614e7 (helm profile routes).
  - PR 3: 412abb72 (213/213, every helm provisioning and host-edit test; their logged panics are the harness's scripted
    ones).
  - PR 4: cedd5842, ef641e66 (all 16 end-to-end restart-with tests) and 0d138e9f (70 of 71, both new restart-with tests
    passing; the one failure is the pre-existing test above, reproduced in 167c195f, 5c3a8e0b and bf5eb5d1 and on main
    in 5690a913).
  - PR 5: c3de0ff0 (menu tests).
- The test-sleep check: zero unannotated delays. It first timed out twice under heavy machine load, then passed.
- `dprint check` on changed Markdown and the changelog format check.

Skipped:

- The full workspace battery, doctests, the browser suite and the desktop checks. Each PR's change is confined to one
  boundary check, and the tests around each one were run. The browser side changed only comments and a label-shortening
  helper that has unit tests.

### Review gate

Each PR had a fresh-context Opus 5.5 review at high effort. None found a defect in the production code except PR 3's,
and every finding was addressed except one nit:

- **PR 1:** the new comments overstated that the helm keeps `..` tab ids out (corrected; see above). A test could not
  tell a refused list from a filtered one, an existing test missed the new `..` case, and two docs, including
  SPEC_impl.md, still listed the old rule. All fixed.
- **PR 2:** the test's assertions were tightened to the exact refusal, and creating a profile is now covered too.
- **PR 3:** the release-time check could leave a lock entry behind if a waiter gave up at the wrong moment. Fixed: a
  refused attempt now runs entirely under the map's lock, and lookups sweep out unused entries. One nit, about making
  error handling symmetric, was declined: the release path must tolerate an earlier panic, and the lookup path keeps the
  module's usual pattern.
- **PR 4:** the first test could not reach the bug, because a plain session was refused for another reason before the
  fix. Replaced with an end-to-end test on a real resumable session, which checks a refused request leaves the agent
  running and the settings unchanged and that a fresh supervisor still loads the session. The message matching and one
  code shape were also tightened.
- **PR 5:** both new test fixtures were corrected to actually exercise the cut-inside-a-marker and nearest-`<` cases.
