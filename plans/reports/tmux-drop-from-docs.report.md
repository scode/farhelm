## What this was about

You asked for tmux and xterm.js to be removed from Farhelm completely, in four chained plans, and said the point was not
only deleting references but making sure no complexity stays behind that exists only for tmux, for xterm.js, or for
having had two kinds of terminal for a while. This is the fourth and last plan. The first three removed tmux and
xterm.js from the running product, from setup, provisioning and releases, and from the test tooling and agent
instructions, each updating the specs and docs for what it changed. This one brings the rest of the written material in
line and audits what is left.

Nothing in the product behaves differently; one refusal message and a few log lines are reworded (below). After this
stack:

- **The specs describe one kind of terminal.** Both specs still described tmux mechanics as current (reading an agent's
  window title with a tmux command, a tab's cleanup settings kept on its tmux window, tmux queries during a supervisor's
  orderly stop), compared the browser's terminal with xterm.js, and justified rules by what the old terminals did. They
  now describe terminal hosts, checked against the code. The implementation spec's tmux section is renamed "Ending the
  old private tmux server" and covers only that startup step; its paragraphs about the desktop window and about setup
  moved to the desktop and provisioning sections, and the general interface rules (sidebar width, sounds, text size, the
  quick switcher and keyboard shortcuts) left the terminal widget's section for one of their own.
- **The docs site and `docs/`.** Pages that still said what works "in an old terminal" or "in old and new terminals
  alike", or reassured readers that Farhelm does not need tmux, are rewritten or trimmed. The architecture diagram in
  the README is redrawn: it showed every session inside a private tmux server; it now shows a terminal host per
  terminal, the supervisor's emulator and the grid stream to the browser. The New terminals page stays at its URL, since
  released changelogs link to it, and now speaks of old terminals in the past. Two Codex investigation write-ups that
  traced the tmux and xterm.js input path are kept as dated records with a note at the top.
- **Code comments**, in the browser's terminal component, the browser tests, the notes on the vendored terminal
  libraries and the Rust crates, no longer explain choices by xterm.js or tmux. Where a value had no reason beyond
  "terminals looked like this before the grid", the comment now says exactly that rather than inventing one.
- **The final audit** searched the whole repository; every remaining match outside frozen material is either fixed or
  kept on purpose for a reason listed under "things you should know". It also turned up code shaped by tmux or by two
  kinds of terminal, which this plan does not change; that is under the open questions.
- **The review queue**: ten findings whose subject no longer exists are removed and three are narrowed to the part that
  still applies. **TODO.md**: the entry for this effort and the documentation entry you decided against (that agents and
  tabs see no `TMUX`) are gone, and five entries lost their tmux and xterm.js wording without changing what they ask
  for.

## Things you should know

- **What still mentions tmux or xterm.js, on purpose**, so a later search is not alarming:
  - the startup step that ends an older release's tmux server, its tests, the old database rows it converts, its
    notifications, and the refusal of a create an older release left half done in tmux;
  - the supervisor database's schema 32-34 history and the protocol version history (upgrades from older releases rely
    on them);
  - everything about a tmux the user runs themselves (no `TMUX`/`TMUX_PANE` passed on, startup files that start tmux);
  - the Linux Update step that deletes the tmux build earlier releases installed, and uninstall's residual for an old
    server on a host that never updated;
  - the few tests that still start a real tmux server (to check that startup step), their helpers, and the tmux the
    release gate, Tensorlake template and deflake sweep install for them;
  - code ported from xterm.js and VS Code with its attribution, and "xterm" meaning the X terminal
    (`TERM=xterm-256color`, the xterm family's key and mouse encodings);
  - the vendored libraries' per-patch records, which cite what xterm, tmux and xterm.js do as reference behavior for
    reports to upstream;
  - the agent screens captured in tmux before November 2026, which status detection is still tested against, because an
    agent in a terminal host started before 0.36.0 still has a tmux-like environment and draws those screens;
  - dated records: investigation write-ups, the bug burndown, closed watch items, queue findings' history, unreleased
    changelog fragments, release history in `releasing/`;
  - identifiers and test names with tmux in them (code; see the open questions), and the validation inventory's
    descriptions of the tests that cover database rows for sessions an older release created in tmux ("old-kind rows").
- **Behavior a spec sentence used to misdescribe, now described as it is.** When the supervisor itself was started
  inside the user's own tmux, the background preparation of spare checkouts runs the user's login startup files with
  that `TMUX` still set, while sessions' terminals never get it. The spec used to call the two the same. Nothing goes
  wrong for a user that I know of, and keeping it may be what stops a "start tmux unless `TMUX` is set" line in a
  startup file from firing in that background shell, which has no terminal. The alternative is to drop `TMUX` there too,
  which would let such a line fire in a shell with no terminal. I recommend leaving it as it is.
- **A user-visible message changed wording.** Retrying a session an older release left half created in tmux is refused,
  as before; the refusal no longer says "this supervisor creates only new terminals" but that it starts every terminal
  in a terminal host. A few supervisor log lines changed wording the same way. No changelog entry was written: the
  meaning is unchanged and only the wording differs, and only an install updating straight from a release that still
  created sessions in tmux, with such a create left half done, can see it. Add a fragment if you want reworded messages
  recorded.
- **One of your rules changed.** The website's editorial rules called "tmux session" jargon and allowed tmux on a page
  only where the reader installs or configures it. That was about Farhelm's own tmux, which nobody installs now, and it
  would have forbidden sentences about the reader's own tmux, so the entry now reads "**pane**, **socket**: say the
  session's terminal". Revert it if you want a tmux rule kept.
- **Review findings removed for reasons other than tmux.** One was about recognizing a user's tmux server by its program
  name; an earlier fix (the one that lets jobs a user's own tmux moved out of a session survive Stop, Restart and
  Delete) removed the code that recognized the user's tmux server, and two sibling findings, but missed this one.
  Removing it departs from the plan, which said it stays because it concerns the user's own tmux; its subject is gone.
  Seven were about the creation code and test fixtures of the legacy container sessions, which an earlier change retired
  (the newer container environments are separate and keep their findings). Their text mentioned tmux, so the audit's
  search matched them and they were checked like the rest.
- **Not changed, because the plan fences it:** the vendored libraries' own patch comments still give tmux or xterm.js as
  the reason for three patches, and one says "new terminals". Changing them means changing a patch file and its vendored
  tree together, which the plan does not allow; see the follow-up below.
- **A cosmetic gap in the rustdoc build, on main already:** building the protocol crate's documentation with warnings
  denied fails on three broken links unrelated to this work (a follow-up below).

## Open questions and possible follow-ups

- **Code that exists only because of tmux or the two kinds of terminal.** None of it was touched; most of it changes
  behavior or tests if removed. My recommendation for each:
  - A small follow-up: Farhelm shows a notification on a session when its launched agent has not reported its identity
    65 seconds after the user's first Enter, and does not count Enters inside a paste. To find pastes it scans typed
    input for paste markers, because xterm.js sent them that way. The grid sends pastes as messages of their own, so
    this scan may never match now; removing it changes nothing if that holds, which the follow-up would check.
  - With phase 3 (the TODO entry for removing the startup step that ends an older release's tmux server): Delete runs an
    extra scan of every process on the host, looking for the session's markers, for every session that has ever been
    restarted, because an earlier release's Restart could move a session out of tmux and leave tmux tabs nothing lists.
    Sessions restarted since then pay for it too; its cost was not measured here. It can go once no session restarted
    under such a release can remain, which is what phase 3 waits for.
  - With phase 3: the tracking of each session's terminal kind and the paths that read it, which exist only for old-kind
    rows; the many test fixtures that still create old-kind rows by default; and the test-state tooling for real tmux
    servers (about 3,000 lines), which now serves the four tests that check that startup step.
  - Leave as they are: status detection reads at most 16 terminals' screens per round, a number sized for a tmux
    subprocess per terminal, and the prompt delivery timeout (10 seconds), which matched the old terminals' input
    budget. Neither has a reason of its own now, but nothing suggests a better value.
  - Check and remove with phase 3, test only: in the session lifecycle tests, a half-second pause before an agent exits,
    which tmux needed, and an extra error a test of Attach racing Delete still accepts, which tmux could produce. The
    comments now say nobody has checked whether terminal hosts need either.
  - Rename whenever that code is next changed: the "pane" vocabulary across the supervisor and tests, the list of tmux
    variables Farhelm withholds from agents and tabs (`PRIVATE_TMUX_ENV_VARS`, now about the user's own tmux), and a few
    test names that say "new terminals" or "never in tmux".
- **The vendored libraries' own patch comments** still give tmux or xterm.js as the reason for three patches, and one
  says "new terminals". I recommend rewording them in the next change that touches those patches, not on their own.
- **Three broken documentation links in the protocol crate**, on main already, make its rustdoc build fail with warnings
  denied. A one-line follow-up fix; nothing depends on it.
- **Values kept only because terminals looked that way before the grid**: the selection color, pointer shapes, the
  14-pixel scrollbar reserve and 4-pixel padding, the cursor blink, the scrollbar's fade, and the 10 MiB limit on a
  program's clipboard write. The comments now say that. I recommend leaving them unless you know a better reason.
- **`docs/codex-input-investigation.md` and `docs/codex-replace-investigation.md`**: kept as dated records because
  nothing else holds their evidence. Delete them if you would rather not keep tmux-era write-ups in `docs/`.
- **`plans/.agents/close-reset.md` still names `.ci-tmux/`** (where test runs used to keep their own tmux build) among
  the build caches a reset keeps (from the previous plan's report; plans cannot edit `plans/`). Fold it into the next
  planning change.
- **The renamed section**: nothing outside the specs cited "Terminal substrate: private tmux server" except one code
  comment, now updated; older plans and reports in `plans/` still cite the old name, and no alias table covers spec
  headings. I recommend leaving it: those are dated records.
- **The Add a remote host screenshot** still shows the old tmux upload step (noted by the second plan); recapture it at
  the next docs screenshot refresh.

## The PRs

- [#3381](https://github.com/scode/farhelm/pull/3381/changes): the specs describe one kind of terminal.
- [#3382](https://github.com/scode/farhelm/pull/3382/changes): the docs pages, `docs/` and the architecture diagram.
- [#3383](https://github.com/scode/farhelm/pull/3383/changes) (the plan's comments step, split in two): comments in the
  browser's terminal component, the browser tests, the vendored libraries' notes and the terminal host's spec (wording
  only).
- [#3384](https://github.com/scode/farhelm/pull/3384/changes): comments and message wording in the Rust crates.
- [#3385](https://github.com/scode/farhelm/pull/3385/changes): the audit's fixes, the review queue and TODO.md.

## Checks run, reused and skipped

This is a documentation and comment change, so per the repository's rules no full test suites ran. Two narrow test runs
did: the JS unit tests, because comments inside browser-side scripts were edited, and the two tests of the one
user-visible message.

- The docs website build, after the docs changes and again after their review fixes: passed, so every internal link and
  anchor resolves.
- `dprint check` on every changed Markdown and source file, and `python3 scripts/check-agents-md.py` (instruction files
  and heading citations): passed.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` on the top of the stack (#3385), which
  covers every PR's Rust edits, and earlier on #3384 before and after its review fixes: passed. Clippy compiles every
  test, so the reworded assertion messages compile.
- Rustdoc for the touched crates: no warning on any line this stack added. With warnings denied the protocol crate fails
  on three broken links that are already on main.
- The JS unit harness (`node --test` in `crates/farhelm-ui/js-tests`) on the top of the stack, through the recorder: 527
  of 527 passed (`e70351d1`). `node --check` on every changed script.
- The two tests that check the reworded refusal, through the recorder
  (`cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` filtered to
  `a_retried_fresh_checkout_interrupted_in_tmux_is_refused_before_its_mkdir` and
  `a_retry_of_a_create_interrupted_in_tmux_is_refused`): 2 passed (`2fb3b105`).
- `python3 -m py_compile` on the three changed Python scripts (docstrings only).
- The architecture diagram was rendered and inspected for overlap.

Reused: none. Skipped: the Rust, browser and desktop test suites (comments and wording only; the one changed message has
its tests above), and the website preview (unattended run). Pages changed: "Agent wrappers"; "Uninstall"; "New
terminals"; "Stop, restart and resume"; "Tensorlake sandboxes"; "Work in a session".

## Review gate

Every PR had the required review by a fresh Opus 5.5 agent at high effort, and every finding was fixed:

- #3381 (specs): 11 findings, all wording: a code comment citing the renamed section, a sentence that newly claimed the
  background checkout preparation and sessions treat `TMUX` the same, Delete's sweep rule stated too narrowly, and
  leftover "pane" wording, among others.
- #3382 (docs): 6 findings: the fixtures note claimed the wrong environment detail and release range, the diagram still
  listed the supervisor's long-removed profiles, an unneeded sentence, "new terminal" left on the New terminals page,
  and two small ones.
- #3383 (browser-side comments): no behavior change; 10 findings: a third copy of a stale xterm.js rationale, tmux
  "window" and "dead pane" wording, generated table headers still saying "new terminal", and wording slips.
- #3384 (Rust comments): no behavior change beyond the message wording; 9 findings: a test comment citing the renamed
  spec section, leftover two-kinds wording, and comments that stated an unchecked reason, among others.
- #3385 (audit): no behavior problem; 7 findings: one place the audit's fixes had missed (a browser test comment), three
  instruction-file lines the audit had not classified (they describe the old-kind rows' tests and stay), the same
  provenance phrase rewritten in one place but not two others, a comment naming only `TMUX` where `TMUX_PANE` is handled
  the same way, a rule in the review queue's filter (the rules triage uses to set incoming findings aside) that still
  said "new terminal" (now just "terminal"), and nits.

Separately from the reviews, my own audit search had suggested rewording one queue finding's summary ("the new terminal
transcript"); I left it, because it is that finding's own dated text. Every commit message went through cold reads (at
most three each).

### Landing

#### What landed with it

Nothing else landed in this round. Since this stack was built, main gained only plan-queue changes, so nothing could
interact with it. An independent reviewer read the whole stack against main with one question above the others: whether
any edit dropped or weakened a claim about the product that is still true while removing tmux wording. An earlier plan
in this series did that with the installer's "Your sessions keep running.", which you had reverted (#3375). The review
found one such sentence (the first fix below). Everything else it checked held:

- The specs still say that terminals keep running across supervisor restarts and updates, that uninstall ends no
  terminal, and that a stopped session's terminal stays viewable.
- The rewritten spec mechanics match the code: how an agent's window title is read, a tab's cleanup settings, the
  supervisor's orderly stop, Delete's extra process scan for a session that was ever restarted, and the fact that the
  background preparation of spare checkouts keeps a `TMUX` the supervisor itself was started with.
- The code PRs change only comments and wording. Besides the reworded refusal (retrying a session an older release left
  half created in tmux) and log lines the report names, the only other non-comment changes are test failure messages,
  one browser-side test name, the supervisor crate's package description, and the header lines of the checked-in
  keyboard tables, which still match the script that generates them.
- No frozen material is touched.
- Each of the ten removed review-queue findings is about something no longer in the code, and none has a pending triage
  outcome.

#### Fixes made while landing

- **The Tensorlake sandboxes docs page had lost a sentence that is still true** (#3382). It used to say "Older hosts
  also need **Update** so their Farhelm can deliver the token to new terminals; old terminals do not receive it." The
  stack removed the whole sentence, but only its second half was about tmux: a host that has not been updated since
  GitHub-token delivery shipped (v0.37.1) still needs an Update before private checkouts there can use the token, the
  spec still says so, and no other docs page did. The page now says "Older hosts also need **Update** before their
  Farhelm can deliver the token."
- **The implementation spec gave a misleading reason for one terminal rule** (#3381). In the section on the supervisor's
  terminal emulator, the rule for a lone zero-width sign at the right edge with wrapping off used to cite tmux. The
  stack had reworded it to "kept as it is, like the two exceptions above", and those two exceptions are kept so that
  saved terminal contents stay sufficient, which is not this rule's reason. The sentence now says what the stack says
  everywhere else a value has no reason of its own: this is how Farhelm's terminals behaved before terminal hosts
  replaced tmux.

#### Things you should know that the report does not say

- **Two true sentences about tmux were removed and I did not restore them**: "Farhelm does not need tmux, on the Mac or
  on any host" in `docs/install_uninstall.md`, and tmux in the Mac uninstall page's list of things uninstall leaves
  alone. They only reassured readers about tmux, and the docs site still says Farhelm needs no tmux on the New terminals
  and Add a remote host pages. Say if you want them back.

#### Checks

The stack changes documentation, comments and wording, so no whole test suite ran (Rust workspace, browser or desktop);
the runs below are narrow ones, all local.

On the stack as delivered, which already sat on main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `python3 -m py_compile` on the
  three changed Python scripts, `node --check` on the changed browser-side scripts and their tests, and
  `cargo clippy --all-targets -- -D warnings` (which compiles every test with its reworded messages): passed.
- The JS unit harness (`node --test` in `crates/farhelm-ui/js-tests`), which also compares the checked-in keyboard
  tables with what the browser client computes: 527 of 527 passed (`57f880c0`).
- The docs website build (`bun run build`): passed, every internal link valid.
- The two tests of the reworded refusal (`cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` filtered
  to `a_retried_fresh_checkout_interrupted_in_tmux_is_refused_before_its_mkdir` and
  `a_retry_of_a_create_interrupted_in_tmux_is_refused`): 2 passed (`ffac0fad`).

After the landing fixes, which changed two Markdown files: `dprint check`, `python3 scripts/check-agents-md.py` and the
website build again: passed.

Reused: nothing. Skipped: the Rust workspace, browser and desktop suites, since nothing they exercise changed behavior;
and the website preview (unattended run). Pages changed by the fixes: "Tensorlake sandboxes".
