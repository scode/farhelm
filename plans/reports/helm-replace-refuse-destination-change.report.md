### What this was about

Replace starts a new session in place of an old one and deletes the old one. An earlier change made the desktop app's
**Replace with** keep the session's folder and checkout fixed (to start somewhere else, you use Clone), and SPEC.md was
updated to say so. The helm's own Replace API, which the desktop app calls, still accepted a request naming a different
folder or a new managed checkout. So a script calling the helm directly could still do what the app no longer offers,
and the helm kept a whole "replace into a fresh checkout" code path alive for it.

You decided the helm should refuse both: a request asking for a managed checkout is refused before anything else
happens, and a request naming a folder other than the session's own is refused once the helm has looked up the session.
You also asked for the now-unneeded fresh-checkout Replace code to be removed.

What was built: exactly that. A Replace with that asks for a managed checkout is refused with a "bad request" answer
(HTTP 400) before the request goes to the host; one that names another folder is refused with a "conflict" answer (HTTP
409), matching how a request naming another host was already refused. Both messages point to Clone. Nothing is created
or deleted in either case. A Replace with in the session's own folder works as before. The fresh-checkout Replace code
is removed. SPEC.md, SPEC_impl.md and the API documentation in the code are updated, the earlier change's unreleased
changelog entry gets a note, and the TODO entry is removed.

### Things you should know

- **More was removed than the plan listed.** Taking out the fresh-checkout Replace left one more piece unused: a
  separate list of recently used repositories, kept for a fresh checkout started inside an existing sandbox or container
  session. Only that Replace ever added to it. The code and its test are removed, and one SPEC.md sentence that
  described the launcher's wording for such a checkout is dropped. The database table behind it stays in place, unused,
  because removing a table needs a database migration. The review found this.
- **The folder must match exactly as text.** A folder that means the same place but is written differently (a trailing
  slash, say) is refused. The desktop app always sends the session's folder exactly as the helm lists it, so this only
  affects scripts calling the helm directly.
- **Nothing changes for the desktop app or the `farhelm` command line.** The app never sends a different folder or a
  checkout on Replace with (the review checked both of the app's Replace with paths), and the command line has no
  replace command. The changelog note says so, so whoever writes the release notes can drop it.
- **The release note.** The plan said none was needed. The earlier change this completes has not shipped in any release
  yet (it landed after 0.46.0-rc.2), so its changelog entry got a context note rather than a second entry.

### Open questions and possible follow-ups

- Possible follow-up: drop the now-unused database table for the separate repository list (above) in a later database
  migration. Nothing adds to it any more; rows already there stay, so leaving it costs only a little space.

### The PRs

- #5142 `fix: refuse a helm Replace that changes the destination`: the two refusals, the removed code, tests, SPEC.md,
  SPEC_impl.md, the changelog note and the TODO entry's removal.

### Checks

Run on the change rebased onto the latest main. Since the work started, main gained eight changes, among them one that
removed an old sandbox route from the same part of the helm. None of them touches what this change does.

- `cargo clippy -p farhelm-helm -p farhelm-proto --all-targets -- -D warnings`: clean.
- Through the test-run recorder (run d21d7a4c-671e-4351-b586-1e8174dccae5, 185 passed, no skipped results):

  ```
  cargo nextest run -p farhelm-helm --lib -E 'test(/^sessions::tests::/) | test(/^tunnels::tests::replacement_closes_old_tunnel_after_the_new_session_exists$/) | (test(/^store::/) & (test(/repo/) | test(/github/)))'
  ```

  That is every helm session test (including the new refusals: a checkout request refused even when the rest of the
  request is also malformed, a different folder refused with nothing created and the old session kept, and a same-folder
  Replace with still working), the tunnel test that follows a Replace, and the helm's stored repository-history tests.
- `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on the changed Markdown, and the changelog
  format check: all clean.

Earlier runs of the same tests, before the review's changes and before the rebase: one failed on a mistake in the new
tests' own setup, fixed straight away and not a flaky test (bd84ce42-4247-469a-aba2-e1e941408bc9); later ones passed
(c4b90767-4b9a-4649-b39a-249101966752, 9c754e4e-04b7-4d0b-8aba-9c81d75fb5d5). They are superseded by the run above.

Skipped: the workspace-wide Rust run, the end-to-end suites and the desktop app's tests. No desktop or command-line code
changed, and the review confirmed the app never sends what is now refused. The plan asked for the app's Replace callers
to be checked; that was skipped because the only code the app shares with this change is a documentation comment. No
earlier checks were reused.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs, confirmed the desktop app cannot trip
either refusal, and judged the order of the checks and the error answers sound. Its findings were:

- the separate repository list (above) had become unused, and the first draft's explanation for leaving it in place was
  wrong;
- the description of the Replace request that client authors read did not state the two new rules;
- the claim that the checkout refusal comes before every other check on the request was untested;
- leftover unused setup in three tests, and one test without an explanation of why it matters;
- an overlong test name.

All were fixed, and a verification pass by the same reviewer confirmed them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round: ntfy alerts for waiting and blocked agents,
with `farhelm agent notify` and, while the Agent is blocked switch is on, an instruction sent with every start and
Restart. It also gained a fresh start for a Claude session restarted before its first prompt, a recipe's Delete question
staying open on Keep editing, and a backoff for an exited terminal tab's failed cleanup. Replace still passes the new
blocked instruction to the session it starts, and Replace never goes through the supervisor's Restart, so the Claude
change does not meet it. In the same round, after this plan, three more landed: agent-delete-notices-bell-alignment
(wording of an agent Delete's cleanup warnings), marked-right-click-cancel-focus (where focus goes after cancelling a
bulk Stop or Delete) and stop-ambiguous-failed-restart-create (Stop's refusal wording). None touches Replace. No version
changes: the helm only refuses more requests, the wire between the helm and hosts is unchanged, and the unused database
table stays, so no migration is needed.

An independent reviewer and I read the change and found nothing to fix. The roughly 1,500 changed lines in the helm's
session tests are mostly test loops collapsed once the fresh-checkout Replace no longer existed, with about 250 lines of
new tests; not churn. The two new refusal tests check what they say.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing changed by the landing itself, but the landing's review found that the report overstates a point:

- **The desktop app can hit the folder refusal, briefly.** The report says the app always sends the session's folder
  exactly as the helm lists it, that nothing changes for the desktop app, and that its review confirmed the app cannot
  trip either refusal. But right after a session starts, its host settles the session's folder and the listed folder can
  change once. A **Replace with** request the app sends in that brief window, from the earlier listing, is refused with
  a readable message, and a retry once the list refreshes works. The app's ordinary Replace with is unaffected. And a
  desktop app from an earlier release talking to this helm, which still offers another folder or a fresh checkout in
  Replace with, now gets a refusal naming Clone; nothing is created or deleted.

#### Worth knowing

- **The desktop app still has code for the removed case, and a changelog draft still announces it.** The launcher's
  wording for a new managed checkout inside an existing sandbox or container session, and the Replace path that chose
  it, can no longer be reached. The unreleased changelog draft `replace-checkout-lifetime.md` still announces that
  launcher message, so release curation should drop it. Possible follow-up: remove the dead code. Likewise, the draft
  `replace-with-keeps-destination.md` says only scripts are affected; an app from an earlier release is too (see above),
  though curation can still drop that paragraph.
- **The report's "Only that Replace ever added to it" is not quite right.** A create through the helm's API (not the
  app) of a managed checkout on a host a sandbox or container owns also added to that separate list of recent
  repositories, and no longer does. The cost is a missing suggestion. SPEC_impl.md's list of such creates leaves this
  case out.
- **A narrow upgrade edge.** A fresh-checkout Replace (which released versions of the app offered) whose reply was lost
  exactly across the upgrade cannot be retried after it, so the old session stays next to its replacement; both are
  visible and you can delete one. The review judged it acceptable.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(helm-replace-refuse-destination-change, agent-delete-notices-bell-alignment, marked-right-click-cancel-focus, then
stop-ambiguous-failed-restart-create), so each plan was tested together with the others and with the previous round's
changes, which none of the executors' own runs included. The four plans touch the helm, the supervisor, the command
line's end-to-end tests and the desktop app's session list, so the whole Rust test suite ran. The SSH test worker was
still unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app that runs by default (`cargo nextest run --workspace`,
  built once, split in three and run through the recorder), recorded runs 2021dc06-f8a9-499f-a932-5b205c06b17a,
  830feae8-2980-4f70-8213-1864c2a3a428 and ce9cbcb2-c7c1-45e1-bcea-70e2a57a9224: 9,186 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache; a boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and no plan in this round touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  aa311d19-8ca8-4224-b6ca-8af001538521, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  5692442c-6eba-4f7a-84ea-a783f3917c82: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include the session row's menu, Replace, Stop, Delete and Mark read; none uses marked sessions.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  852c4855-d075-47b1-8cbb-dd08f75e9f93: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the opt-in tests, among them the real-agent drift tests, which spend vendor turns; the
installer and provisioning suites, since no plan in this round touches them; and any run on a Mac, since none was
available to the landing.
