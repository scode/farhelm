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

### Landing

#### What landed with it

It landed in one round with version-dblclick-flake (a browser test waits for the terminal's focus before double-clicking
the header version) and cmd-click-images (more file types open on Cmd-click by default); neither touches what this plan
changed. Before the round, main had gained sandbox-suspend-overlap (#3339, #3341), which lets sandbox Suspend and Resume
run beside launches and changed the same parts of the web app: how a launch keeps other actions waiting, the session
actions, and the launcher. This plan was written before that landed. A clean rebase proves nothing when the same code
changed, so the landing's review and an independent one read the combined code on purpose. They found no interaction:
this plan's two fixes (forgetting a finished Restart once the session list shows the new run, and Replace with's
progress cleared by the request that owns it) do not rely on anything that plan changed or removed. While the round ran,
a docs refresh (#3349) and a change to a docs-screenshot test (#3347) landed on main; neither touches this plan's files.

No fixes were made while landing.

#### Possible follow-ups

- The independent review noted one ordering it judged unreachable in practice: if a session-list reply arrived before an
  open session's view had noticed a just-finished Restart, the view would take the replaced run's kind of terminal
  instead of the new run's (the report explains what the remembered Restart is for). A network round trip separates the
  two, so it was left as is.

#### Checks

The round's three plans were checked together, stacked in queue order on main (session-action-leftovers, then
version-dblclick-flake, then cmd-click-images; their TODO.md conflicts were resolved by keeping each plan's removal of
its own entry). Browser runs were on Tensorlake sandboxes at the maintainer's request; the sandboxes run Node 24.16
where the development machine runs 24.20, which these tests do not depend on, and each shard is recorded in the
recorder's mode for arbitrary commands.

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build (`bun install --frozen-lockfile` and
  `bun run build`), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` passed.
- `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui --lib`: 2,978 passed (run
  `64f04ec9-18bc-4bb4-bdd1-fb2503b7cbb9`).
- The whole browser suite, `npx playwright test` split by project over ten sandboxes, because three plans' interactions
  were checked in one run and session-action-leftovers changes how every session view keeps a finished Restart: 3,296
  passed, 14 skipped, none failed (runs `4f802ef7-91db-4b47-9622-41cf69ad4216`, `ff5047d5-a356-4d6d-bd9e-e6d79ddb5e00`,
  `97413f07-ea76-496b-8a26-64ff1045e28b`, `ee9ea862-1e4c-437f-bedd-f90e8c07713f`,
  `8e430cf6-69d0-4c89-9e08-09f390b77eea`, `5dd8460f-f24d-4845-94ea-23b92f1697dd`,
  `87bed0c7-b106-4e86-9a62-9122c45e4de0`, `7f818c77-32af-40f4-afeb-2dc08cc60aaf`, `8bb6cc96-5151-42f9-a4c4-9385246c5f1d`
  and `517ea6a3-9db7-4f5b-ac17-f5139f5ffae6`). The 14 skips are as many as in earlier whole-suite runs.

Main differs from the tested tree only by the docs refresh (#3349) and the docs-screenshot test change (#3347), which
landed while the round ran; the docs refresh and cmd-click-images both edit the session list's docs page, in separate
paragraphs, and the website was not rebuilt with both edits together. Skipped: the workspace Rust tests beyond the three
changed crates' library tests (nothing else reads what changed), the old-terminal browser set (nothing here touches old
terminals), and the desktop app itself (the Cmd-click list is shared code the library tests cover).
