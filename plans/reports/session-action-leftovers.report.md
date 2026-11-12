### What this was about

The landing review of nonblocking-session-actions found two pieces of session-action state that could outlive their
purpose.

**A Restart remembered too long.** When a session's Restart finishes while you are looking at another session, the app
remembers the successful reply, so that opening the session soon after shows its terminal even though the session list
still calls it interrupted (the list catches up only when the helm next checks the restarted run). Only the open
session's own refresh ever forgot that reply, so it lasted until the session was next opened. If the host rebooted again
in between, opening the session trusted the old success and tried to attach to a terminal the reboot had destroyed,
instead of showing the interrupted card with its Restart offer.

You decided on the planning page that the remembered Restart is forgotten as soon as the session list shows the session
in any status other than interrupted, or its host as unreachable, while no action on the session is still running (the
rule the open session already used), and you accepted the remaining gap described under "Things you should know".

**"Replacing…" with no cleanup.** Replace with shows "Replacing…" on the source session's row and at the top of the
session page while its request is out. It removed that only once the reply came back, so a request abandoned before then
would have left it on the source for as long as the page was open. Plain Replace already ties its progress to a cleanup
guard. Nothing abandons a Replace with request today (the launcher cannot be closed while it is sending), so this part
is defensive.

Both are now fixed as planned: the session list forgets a remembered Restart under your rule, and Replace with uses
plain Replace's guard.

### Things you should know

- **The gap you accepted remains.** If the host reboots again before any refresh of the session list has shown the
  restarted session running or its host unreachable, opening the session still tries to attach to the destroyed
  terminal. Telling that apart from the moment right after the Restart would need the helm to report reboots.
- **Execution found a timing problem the plan did not expect, and fixed it narrowly.** A list refresh that was already
  on its way when the Restart's reply arrived can still land after the reply (the list only drops a refresh older than
  one it has already shown). For a Restart of a session that was running or had exited, that refresh shows the replaced
  run, which is not interrupted either, so it would have made the app forget the Restart at once. The cost would have
  been the terminal kind: the remembered reply also says which kind of terminal the restarted agent now runs in, and a
  session opened right after would have started with the old run's kind. When a Restart moved an agent from an old
  terminal to a new one, the page would first set up the old kind of terminal and switch once the terminal answered,
  rather than going straight to the new one. The app now notes the newest list refresh started when the reply arrived,
  and only a later refresh may forget the Restart. This is not the fencing of list refreshes the plan ruled out: other
  refreshes are not affected.
- **Filtered lists count too.** The plan proposed applying the rule only to list refreshes the app treats as speaking
  for the whole fleet. In the code that excludes every refresh made while any list filter is applied, so the bug would
  have stayed whenever a filter was on. A refresh that shows the session is evidence about that session whatever else it
  leaves out, the same reasoning the list already uses to confirm renames, so every refresh counts.
- **A brief, narrow gap through the quick switcher.** The quick switcher opens a session from its own snapshot of the
  list, taken when the switcher opened. If a session's Restart finishes and the list forgets it while the switcher is
  open, and you then pick that session from the switcher, the session can show the interrupted card for a running agent
  until its own first refresh, a moment later. Before this change it would have shown the terminal. Closing this would
  mean changing what the switcher hands over, which was out of scope.
- **Changelog.** The nonblocking-session-actions entry had already been released, so the Restart fix has its own `fixed`
  entry. Replace with's guard has a `none` entry, since nothing a user does reaches it.

### Open questions and possible follow-ups

- If the quick-switcher gap above matters to you, the switcher could open a session with the list's own current copy of
  it when the list has one, instead of its snapshot.

### PRs

- #3345: the session list forgets a finished Restart once it shows the new run.
- #3346: Replace with clears its "Replacing…" progress through plain Replace's guard; removes the TODO entry.

### Checks

Run now:

- `cargo fmt --all -- --check` and `cargo clippy -p farhelm-ui --all-targets -- -D warnings`: clean on both PRs.
- `cargo check -p farhelm-ui --target wasm32-unknown-unknown` (the web build target): clean on #3345. Not re-run on
  #3346, which changes no target-specific code.
- The UI crate's unit tests through the recorder, `cargo nextest run -p farhelm-ui`: 968 of 968 passed on #3345 (run
  622c7c4c), 969 of 969 on #3346 (run f11d900e). The new tests cover forgetting a Restart once a list refresh shows the
  new run (and not on an interrupted, older or omitted one), keeping it before the list catches up, the interrupted card
  coming back after a later reboot, and an older Replace guard leaving a newer request's progress alone.
- `scripts/check-test-sleeps.py`: no unexplained delays.
- `releasing/check-changelog.py format`: passes.

Reused: the test runs above for the final commits. After each review only comments and changelog wording changed, so the
runs made before those fixes still cover the code.

Skipped:

- Browser tests. A browser test for the Restart fix was optional in the plan. Staging it needs a second reboot in the
  existing test fixture, with the list changing back and forth and the Restart reply held until another session is open.
  The unit tests drive the same decision the session page makes when it opens. No browser test was planned for Replace
  with, because the launcher cannot be closed while it is sending.

### Review gate

Each PR had one fresh-context Opus 5.5 review at high effort. Neither found a correctness bug.

- #3345: seven low findings. It agreed with the timing fix and with counting filtered lists. Its fixes went into
  comments and the changelog wording (one comment had wrongly said a restarted session lists as interrupted until the
  helm checks it; in fact the helm keeps the session's previous status). The quick-switcher gap is its finding, accepted
  as described above. Not followed: extracting a helper so the new session-page test shares the page's own setup code; I
  judged a two-line setup not worth a new helper.
- #3346: two low findings, both about the new test's description, fixed.
