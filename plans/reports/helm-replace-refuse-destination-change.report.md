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
