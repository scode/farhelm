### What this was about

The startup Welcome card interrupted the first window and left the main pane with only a short placeholder. The agreed
design puts the introduction in that pane whenever Farhelm has confirmed there are no sessions. Help → Welcome opens the
same introduction on request.

### Things you should know

- The pane offers Start a session, Add a remote host, the Get started guide and Send feedback. Its marks follow the
  built-in agents in the launcher. At the minimum window width the links can wrap and stay centered.
- With an empty fleet, focus starts on Start a session, and Enter opens the launcher. When the first session arrives and
  replaces the focused pane, focus can move to the new terminal. In the Help dialog, a disabled Start a session gives
  initial focus to Add a remote host. Escape returns to Help, and closing a form opened from the dialog returns there
  too.
- Startup can show pending announcements followed by upgrade highlights. The stored legacy Welcome dismissal is carried
  for compatibility. The desktop reads and opens the introduction independently of it.
- The first-session guide now describes the empty pane. The legacy smoke step that opened a second app to check its
  startup card was removed. The existing smoke harness cannot seed a pending announcement at startup, so GUI and Python
  tests cover the override. The live smoke exercises Welcome through Help.

### Open questions and possible follow-ups

The brick-test operator walkthrough still describes dismissing the former startup card. Its automation already checks
conditionally for the card. A future brick-test documentation pass can remove the old instruction.

### Pull requests

- Welcome in the empty main pane. [#5012](https://github.com/scode/farhelm/pull/5012/changes)
- Welcome from Help and the end of the startup card. [#5018](https://github.com/scode/farhelm/pull/5018/changes)

### Checks

- `cargo nextest run --locked -p farhelm-gui --lib -E '<Welcome and startup selection>'` passed all 71 selected tests
  (run `73ca2fe4-efaa-4d6d-b296-9329599e5983`). The selection covered the empty pane, focus handoffs, Help actions,
  announcement ordering and upgrade highlights. `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`
  passed.
- `python3 scripts/check-gui-goldens.py --update` passed all four Linux screenshot jobs on the rebased stack with
  coverage checks for both appearances (run `228d4f9d-8b29-4890-80e9-82916fca65dc`). The four empty-pane pictures
  matched their approved references. The six changes were the Welcome dialog and the two Help menu scenes in both
  appearances. What's New and the New container recipe matched main's updated references.
- `python3 -B` ran `scripts/test-gui-e2e-smoke.py`, `scripts/test-gui-agent.py` and `scripts/test-check-gui-goldens.py`
  (130 tests, run `7740a73d-8b5b-4a9e-ae0b-791d10c4d9d3`). The same interpreter ran
  `scripts/test-gui-screenshot-batch.py` (47 tests, run `e2bae7c8-0d2a-424b-a2c0-c19f6c53df10`).
- `cargo fmt -p farhelm-gui -p farhelm-proto` and `dprint fmt <changed Markdown files>` completed.
  `jj diff --git --from main@origin --to @ | git apply --check --reverse --whitespace=error` passed. Each changed shell
  script passed `bash -n <script>`.
- `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` passed.
  `python3 scripts/check-agents-md.py` reported the existing broken archive citation and no errors.
- Reused `python3 -B scripts/gui-e2e-smoke.py` on `7a1e3c0f` before the final rebase (run
  `61a1b040-bddd-4901-8085-5e04783fb38c`). It passed 48 steps, including Help opening Welcome and Escape returning
  focus. The platform-specific paste step was skipped. The rebase and review fixes preserved the automation IDs and the
  behavior the smoke exercises for Welcome. The new disabled-button case passed the focused GUI run after the rebase.
- Skipped the full Rust workspace suite and an installed Mac run. The scoped tests cover the affected GUI behavior and
  the Linux runs exercised the bridge and screenshots.

### Review gate

Separate Opus 5.5 high-effort reviewers checked source and screenshots for each PR. Source review prompted focus
coverage for the first session and the disabled Start a session action, accessible names for the agent marks, and
corrections to the Help wording and startup documentation. Both source gates cleared after those corrections. Final
screenshot review accepted the Welcome dialog and Help menu in both themes. The narrow pane wraps its links, and the
dialog uses the shared launcher-style header with the standard app glyph.

### Landing

#### What landed with it

Only this plan landed this round. Nothing but plan bookkeeping had reached main since its PRs were based when the round
started; while it ran, a planning PR that queued eight plans for triaged review outcomes
([#4981](https://github.com/scode/farhelm/pull/4981/changes), which also records them in the triage outcomes file) and a
TODO entry about moving agent records off the host ([#5020](https://github.com/scode/farhelm/pull/5020/changes)) landed.
Both change only documents that this plan does not touch, so main now holds the tested tree plus those two. The
landing's independent review checked the points most likely to go wrong and found them sound: the empty pane shows
Welcome only once the session list has confirmed there are no sessions ("Loading sessions…" is unchanged); Help →
Welcome with Start a session unavailable starts on Add a remote host; nothing reads or writes the old card's stored
dismissal any more, and the only protocol change is that field's description; and the smoke step that started a second
app to check the startup card is gone everywhere it was described.

#### Fixes made while landing

All inside [#5018](https://github.com/scode/farhelm/pull/5018/changes), text only. The review raised four should-fix
items: the second and fourth are fixed here, and the first and third are described under the last heading.

- **SPEC.md still listed a Welcome dismissal (second should-fix)** among the preferences every window shares; that
  dismissal no longer does anything, so it is off the list.
- **The screenshot parity record still described the removed card's differences from the old app (fourth should-fix).**
  Its Welcome section now says the startup card is gone and Welcome is a new design that SPEC.md describes.
- **About 20 code comments still said the stored dismissal keeps the card from opening, or that focus rests on New over
  an empty fleet.** They now say the value has been inert since the card went and the test fixtures keep it to avoid
  churn, which is the plan's own decision, and that focus rests on Welcome's Start a session.
- **The changelog draft did not mention that the startup card no longer opens**, which an upgrading user who never
  dismissed it will notice; it now tells the curator.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs as the executor pushed them, on main as it stood at the start of the round (`18d73f22`, only plan
bookkeeping newer than the plan's base; the two document-only PRs named above landed later). The landing's fixes since
change only comments, SPEC.md, the parity record and the changelog draft; after them
`cargo clippy -p farhelm-gui --all-targets -- -D warnings` on the test worker, `cargo fmt --all -- --check`,
`dprint check` on the changed files, the changelog lint and the agent-instructions check passed again.

- The GUI crate's and the protocol crate's tests, `cargo nextest run -p farhelm-gui -p farhelm-proto`, on the test
  worker (recorder run `a75c0ce6-ea79-49b2-838a-91c451fcc74e`): all 3728 passed. Only these crates' code changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `05554055-f125-436f-be2a-8c33bfa893c6`): of its 49 steps, 48 passed and one was skipped by design on Linux (paste),
  including Help opening Welcome.
- The Linux app lifecycle suite, `python3 -B scripts/gui-lifecycle.py`, on the test worker (recorder run
  `cd0685f0-b02f-46bd-b986-5a9c3d2b3d35`): every step passed. It drives the app through the changed automation script.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `42681df9-1365-43b0-8e2a-7e28b8da8305`): passed, including the pictures the plan updated.
- The screenshot batch tool's test, including the committed scene list (recorder run
  `290c34e9-a8b1-4409-965c-34f6af422a72`), the automation script's tests, `python3 -B scripts/test-gui-agent.py`
  (recorder run `fedc5567-8b84-4361-b0d0-436d30964cd1`), and the smoke script's own test,
  `python3 -B scripts/test-gui-e2e-smoke.py`, locally: passed.
- `shellcheck -x` on the two GUI suite runners the plan relabeled, locally: passed.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed (the
  first-session guide changed).
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive, which main has had since an earlier triage record (#4978)
  landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused across the landing's text-only fixes: every run above, since those fixes change no code that runs. The executor's
runs were focused selections, and everything they covered ran again above.

Skipped: the rest of the workspace Rust suite and the desktop app's own tests, since no other crate's code changed; a
Mac run, since nothing changed is Mac-specific; the release brick test, which is a release-time check (its stale
instruction is described below); and CI, which runs only on demand.

#### What the report says that is no longer true

- "Its automation already checks conditionally for the card" (about the release brick test) is only half true; see the
  first point below.

#### Things you should know that the report does not say

- **The release brick test still tells its guest agent to dismiss the Welcome card (the review's first should-fix; I
  recommend fixing it before the next brick test).** The candidate's app-start checkpoint instructs the agent to
  "dismiss welcome with got it" unconditionally, and the brick test's own operator guide says the same; a conditional
  sentence was only added after it. A fresh install of the next candidate shows Welcome in the empty pane, with no Got
  it, so the agent may report that it could not run the step or click something else. The checks of the previous release
  are unaffected, since that release still has the card. I left it alone because it is release tooling outside this
  plan; say if you want a TODO entry.
- **A session you did not start can now take keyboard focus over an empty fleet (the review's third should-fix).** The
  plan added a rule that when the first session appears, its terminal may take focus once the focused Welcome control
  leaves the pane. Before, focus stayed on the sidebar's New. So if a session created from the command line arrives
  while Start a session is focused, the next Enter goes into that agent's terminal, which sits uneasily with SPEC.md's
  rule that things the user did not start never take focus. It follows the existing rule for where focus goes when its
  control disappears, and the window is usually not in front when a command line creates a session, so I landed it as
  built; say if you want focus to go to the sidebar's New instead in that case, as before.
- **Other screenshots of a window with no sessions now show Welcome behind their dialog**, for example the empty Trash
  dialog's. The plan asked the report to say so; it does not, so it is said here.
- **A TODO entry about Tensorlake onboarding still says to update "the welcome card"** so sandboxes are discoverable
  from first launch; that card is now the empty pane and Help's Welcome dialog. I did not edit your entry.
- **Smaller points the review noticed.** One smoke step checks that the Welcome dialog closed only by its name's
  absence, without first seeing that name while it is open; one marks test builds its expected order from the same list
  the code uses; and the first-session docs page has one unevenly wrapped line (the rendered page is unaffected). None
  needs a follow-up on its own.
