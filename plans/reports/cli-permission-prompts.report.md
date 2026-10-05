### What this was about

An agent running in a Farhelm session can use the `farhelm` command to act on the whole fleet: start sessions on any
host, copy a session to another host, rename, stop and restart any session, and spawn a child of its own. Until now none
of that asked anyone. The spec carried this as a temporary exception to the rule that one host must not gain execution
on another, and your TODO entry asked for every such action to ask the user first, with an "always allow from this host"
option modelled on the per-host setting for YOLO launches. The plan added two requirements from the launch-kinds
redesign: an agent's command launch on a host that asks before YOLO launches must not get through on the agent's own
claim that it is not YOLO, and agents should be able to create, edit and delete launch templates once they are behind
the same prompt.

That is what landed:

- **The helm asks before every acting `farhelm` command from a session.** A card appears in the bottom-right corner of
  the Farhelm window naming the session that asked, its host, the action and its target, and for a launch the host,
  directory, agent and its choices (or the full start and resume commands) and whether it is YOLO. Everything the agent
  wrote is shown as labelled values. It has **allow**, **always allow from** the host, and **deny**. The agent's command
  waits up to nine minutes, then is told nobody answered. Listings never ask.
- **A per-host setting, "run farhelm commands from this host without asking"**, in each host's settings next to the YOLO
  one, off for every host, including existing ones after the update. **always allow** turns it on. It goes back to off
  when you adopt a reinstalled host, like the YOLO setting.
- **On a host that asks before YOLO launches, an agent may not start a YOLO launch or any command launch at all**, with
  or without approval; the refusal names the host and the setting that allows it. `--confirm-yolo` is gone.
- **Agents can create, edit and delete launch templates** (`farhelm agent template create|edit|delete`), each behind the
  same card, which shows the whole resulting template including command text the agent cannot see.
- SPEC.md drops the temporary exception; SPEC_impl.md gains a section on how the prompts work; the website gains a page,
  "Approve what agents do", and mentions on the Launch templates, Manage hosts, Start a session and Custom commands
  pages. The TODO entry is removed, as the plan said.

### Things you should know

- **This is a breaking change for agents.** Every acting command now needs a Farhelm window to be open: with none open
  (on the Mac, the app quit or its window closed), the agent is told at once to ask you to open Farhelm. A browser tab
  whose live update connection failed and fell back to polling counts as no window too; I did not add a second way to
  detect an open window.
- **Release notes**: #1608 carries a "breaking" changelog fragment saying what agents and scripts lose, #1609 an "added"
  one for template writes; the other code PRs carry "none" fragments.
- **`farhelm spawn` now always goes through the helm**, including `--inherit-agent`, which the session's own supervisor
  used to answer even with no helm attached. It needs the session to be open in Farhelm, like the other commands. A
  keyed `--inherit-agent` spawn retried across the update is not recognized as a retry: it gets a new card and, if
  allowed, a second child. The plan required the helm to decide; this follows from that, and the card is the safeguard.
  I accepted it; it is recorded in SPEC_impl.md's compatibility section.
- **The YOLO rule for agents is stricter than "YOLO and command launches".** Execution found that a copy of a session
  takes its command lines from the source host's supervisor, which the helm does not trust, so a copied launch could
  carry YOLO command lines under non-YOLO choices. On a host that asks before YOLO launches, an agent launch now counts
  as safe only when its command lines are exactly what this version of Farhelm builds from its choices. As a side
  effect, on such a host an agent cannot copy (or `--inherit-agent` from) a session whose command lines an older Farhelm
  built differently. That affects only sessions started before a release that changed how an agent's command line is
  built, and only those two operations: restarting such a session, and every launch the agent describes from scratch
  (`farhelm agent create`, spawn with launch flags), still work, and turning on the host's "start YOLO sessions here
  without asking" setting lifts it. I did not measure how many existing sessions this covers. I added the rule to
  SPEC.md and the website page; it is yours to confirm.
- **A host can have at most four cards waiting.** While four wait, every other agent request from that host, listings
  included, is refused as "too many in flight". This follows from an existing per-host limit on agent requests; SPEC.md
  already accepted that a host can keep only a few cards waiting (a host that keeps asking is answered by denying its
  cards), and SPEC_impl.md records the listing refusal as part of that.
- **An approval does exactly what its card showed**, which needed two things the plan did not foresee. A template write
  is refused if the template changed (or appeared) while the card waited; this makes agents' template writes the one
  exception to the spec's last-write-wins rule for templates, and SPEC.md now says so. And an approved restart carries
  the launch its card showed to the supervisor, which refuses it if a Restart with changed the session meanwhile; a
  restart is refused before any card, with "retry shortly", in the rare case that the helm's stored copy of the
  session's details is unreadable at that moment (for example written by an incompatible build).
- **Template writes from agents have a few rules of their own**: an edit sets only the fields it is given and cannot
  unset one (so command text the agent cannot see is never dropped); an edit cannot switch a template between an agent
  launch and a command launch; and `--command` needs `--yolo` or `--no-yolo` with it. Delete and recreate is the way
  around the first two.
- **Allowing a card after the agent stopped waiting still acts**, as SPEC.md requires (an approval carries out what the
  card showed even if the asking command was killed). The page says to deny cards you no longer want. When an answer
  arrives too late, the window says it did not take effect and, after **always allow**, that the host's setting may
  still have been turned on.
- **Protocol versions 41 and 42.** The gate and the template writes each change what the helm and supervisors say to
  each other, so after the update every host is offered the usual "update this host" until it runs the new build, as
  with any protocol change. Nothing extra is needed.
- **The prompts are not a sandbox.** Anything running as the session's user can still talk to that host's supervisor
  directly with full authority and act on that host without asking; SPEC.md already leaves same-account isolation out of
  scope. Reaching another host still goes through the helm.
- **Rebase interactions with main**, all resolved: the new hover-text rule (the card buttons and the new switch got
  hover text, and hover help draws on top of the cards); conversation reports leaving the supervisor protocol (a
  credential check that both changes had stopped using was removed); and the Launch templates page being written
  meanwhile (its "agents cannot create templates" sentence now says they can, behind the card).

### Open questions and possible follow-ups

- **Docs screenshots.** The new page has no screenshots of a card, and the Manage hosts page's host settings shot
  predates the new checkbox. Both need the bulk docs screenshot refresh; I did not run it, since it republishes every
  page's shots.
- **The stricter agent YOLO rule** (above): confirm, or ask for a follow-up that vouches for older sessions' launches
  some other way.
- **Declined review suggestions**, for your awareness: a forced-interleaving test proving the template check and write
  share one database transaction (the code shows it; the test pins only the comparison); a refactor so the CLI's verb
  table also builds the template verbs' requests; and a presence signal for browsers on the polling fallback (a browser
  stuck on the fallback gets "no Farhelm window is open"; reloading it usually restores the live connection). I
  recommend leaving all three.
- **The queued plan "agent-retry-by-request"** changes how retried agent creates are matched to their first request.
  This stack added the approval to that path (a retry is asked about like a new request) and removed `--confirm-yolo`
  from what a retry is compared on; that plan should be checked against SPEC.md's new wording when it runs.
- A reviewer suggested recording "card" as a user-facing term in the website's vocabulary, and placing the new page next
  to Manage hosts in the sidebar rather than last in "Using Farhelm". I recommend both; neither is done, since the
  vocabulary list is yours and the order is a judgement call.

### PRs

- #1605 `docs: specify asking the user before farhelm CLI actions` — SPEC.md and the narrowed TODO entry.
- #1606 `feat: add the helm's table of agent requests awaiting the user` — where waiting requests are kept and answered,
  and the per-host setting; nothing asks yet.
- #1607 `feat: show agent requests waiting for approval as cards` — the cards and the host settings switch.
- #1608 `feat!: ask the user before an agent's farhelm command acts` — the gate on every acting command, spawn through
  the helm, the agent YOLO rule, the restart precondition.
- #1609 `feat: let agents write launch templates, with the user's approval` —
  `farhelm agent template create|edit|delete`.
- #1610 `docs: document asking the user before agent actions` — SPEC_impl.md, the website, removal of the TODO entry.

### Checks

Run ids below are the recorder's retained test runs (`scripts/record-test-run.py`), on the final stack or as noted:

- Workspace Rust tests, `cargo nextest run --workspace --exclude farhelm-desktop` through the recorder (run dc83523d),
  at the tip before the last review fixes and the last rebase: 3287 of 3288 passed. The one failure,
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, is a timing assertion under the full battery's
  load in terminal output handling, which this stack does not touch; it passed 10 of 10 when repeated alone (repeated
  run cf51c58e). Treated as a load event, not logged as a flake on one observation; the failed run is retained.
- Browser specs on Chromium and WebKit (run a500d994): spawn, agent-relay, terminal-multihost and the test that checks
  every control has hover text, 106 passed; 2 skipped are the opt-in real-agent spawn test.
- After the last review fixes: helm restart and approval tests (62fb478e, 30 passed); restart precondition, YOLO guard,
  session-request dispatch, spawn and approval tests across proto, supervisor and helm (6abacbb6, 116 passed); template
  tests (bca5321a and earlier runs).
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: clean.
  `cargo check -p farhelm-ui --features desktop`: clean. `cargo fmt --check`, `dprint check`, the test-sleep check (0
  unannotated), and the website build (all internal links valid): clean.
- Each of the first three commits was checked to build and pass clippy on its own before the rebase; the rebase changed
  only the third (hover text), which compiles as part of the stack.

Reused or skipped: doctests (no doc examples changed); desktop runtime and smoke tests (desktop-specific code is
untouched; the cards are shared UI covered by the browser run); installer, provisioning and release-only gates
(unaffected).

### Review gate

Every PR had two reviewers (a Claude and a GPT model), the gate PR (#1608) an extra round, and everything changed after
review a final round. Besides the findings already described above (the copied-launch YOLO hole, restarts and template
writes bound to their card), fixed: cards that could outlive a replaced host connection, and "always allow" reaching a
reinstalled host; the template messages reusing a protocol version; keyboard focus on a card being taken by a terminal;
a late answer claiming nothing was approved; and stale SPEC_impl.md text in several sections. Declined: the three review
suggestions under open questions.

### Landing

Landed on 2026-10-05 (UTC) as six squash commits on main, in stack order: #1605 (the spec), #1606 (the helm's table of
waiting requests and the per-host setting), #1607 (the cards), #1608 (asking before every acting `farhelm` command),
#1609 (template writes from agents) and #1610 (the implementation notes, the website and the TODO entry's removal).

#### What else was on main

Between the commit the stack was built on and the landing, main gained only TODO.md edits and the planning queue's own
files: one TODO entry removed ("Confirm the help menu fix on the Mac") and plan pointers added to two others. None of it
touches code, SPEC.md or SPEC_impl.md, and the TODO edits sit apart from the stack's, so the rebase applied without
conflict. Nothing else reached main while the six PRs merged. No other plan landed in the same round.

One earlier change matters more, and the report's list of rebase interactions leaves it out. Just before delivering, the
executor rebased the stack onto the fix that shows the whole help menu in the Mac app, which moved where that menu opens
in the sidebar. The executor's last full Rust run (dc83523d) and its browser run (a500d994) came before that rebase, on
a stack that already included the rule that every control has hover text and the change that saves conversation reports
as files. Between that full run and the delivery the stack also gained one small fix: an agent's restart is refused,
before any card, when the helm cannot read the session's stored launch details. Targeted runs covered that fix (62fb478e
and 6abacbb6, listed in the report's Checks). No Rust or browser test ran on the stack combined with the help-menu fix.

An independent review by a fresh-context sub-agent, done before anything merged, read that combination instead. It found
no interaction: the menu still sits below the cards, and the cards below hover text; the cards are never inside the
sidebar where the menu now opens; the menu's keyboard focus and the cards' share no code; and the test that requires
hover text on every control is satisfied by the cards' buttons and the new host setting.

#### What the review found for later

The review found nothing outside the six PRs that breaks: by its search, no test, script or page still uses
`--confirm-yolo` in a way that fails, and nothing else calls what the stack changed. It did find text that is now out of
date. None of it was changed during the landing, and nothing tracks it apart from this note:

- **The queued plan "agent-retry-by-request" contradicts what landed.** Its Decision 5 says
  `farhelm spawn
  --inherit-agent` is answered by the session's own supervisor and never the helm; it still treats
  `--confirm-yolo` as part of what a retried request is compared on; and it assumes protocol version 40, where main is
  now at 42. The report says to check that plan against SPEC.md when it runs. Given these three points, it needs
  revising before it runs: an executor picking it up as it stands would build against a design that no longer exists.
- **A pending changelog fragment from an earlier change** (`agent-cli-launch-flags.md`) says `--inherit-agent` runs with
  no helm involved and that agents cannot create or change templates. Both are now untrue, and release curation has to
  reconcile it with this stack's fragments.
- **The website's "Your first session" page** says the host's supervisor answers an agent's `farhelm spawn` and
  `farhelm agent` commands. Those now go through the helm and need a Farhelm window open, so the sentence is incomplete.
- **Four browser tests** (for Restart with and for restarting from the terminal) fake a helm refusal whose text still
  mentions `--confirm-yolo`. They only check that the UI does not show that text, so they still pass.
- **The TODO entry "Let the Mac's supervisor outlive the desktop app"** argued that a supervisor outliving the app would
  keep `farhelm spawn` working. Spawn now needs the helm and an open window, so that part of its case no longer holds.

#### Checks

- Run now, on the final stack after rebasing onto the latest main: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check`, and `python -B scripts/check-test-sleeps.py` (no
  unannotated delays). All clean.
- Reused: the test runs in the report's Checks section. They predate two code changes: the restart fix, which the
  targeted runs above cover, and the help-menu rebase, which only the review above covers. The rebase onto the latest
  main brought only TODO.md and queue changes, and the landing changed no code.
- Skipped: running Rust or browser tests again. Nothing in the code changed during the landing; the remaining gap is the
  help-menu interaction, which the review read and found none.

The landing itself made nothing in the report above untrue. These notes add to it in two places: the help-menu rebase,
which the report does not mention, and a firmer conclusion about the queued plan "agent-retry-by-request" than the
report's "check it when it runs".
