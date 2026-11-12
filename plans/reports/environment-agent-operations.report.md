## What this was about

Before this plan, an agent running inside a sandbox or container session could not ask Farhelm for anything:
`farhelm spawn` and every `farhelm agent …` command were refused outright from those sessions. Agents on ordinary hosts
had no way to start a session in a new sandbox or container either. This plan covered four TODO.md entries: letting
agents in sandbox sessions ask Farhelm to act, Farhelm integration for agents inside containers, `farhelm spawn` and CLI
flags for containerized sessions, and the two approval-card findings from the container review (launch cards must show
the environment target, and such a card must never reach an older page that would show it without the target, so older
pages cannot show it at all).

What you decided when planning (2026-11-08): an agent inside a sandbox or container acts only within its own
environment, under the same approval rules as any host; it can never create a new environment; agents on ordinary hosts
may launch a session in a new environment behind an approval card that always asks and doubles as the creation
confirmation; and an environment stays until its last session is deleted.

That last point turned out not to match what had landed. A day after you planned this, in the sandbox-sessions plan
(which landed first), you decided (2026-11-09) that Delete always destroys the whole sandbox or container. This plan's
first round built everything else and then blocked on the question. Your answer (2026-11-12, recorded in the plan's
Decisions with the landing monitor's adjustments) was: the user's Delete of an environment session goes to the Farhelm
inside the environment as one request, "delete this session, and if it was the last one, refuse every new session from
now on" (a seal), decided atomically there; the helm destroys the environment when the answer is "sealed"; and when the
inside Farhelm cannot be asked, Delete offers the whole-environment Delete with a warning. You also asked that this keep
working once sessions inside an environment can start more sessions while the helm is disconnected; it does, because
only the inside Farhelm counts.

What is built, in seven PRs (listed below):

- **Agents inside a sandbox or container** can now spawn and create sessions in their own environment, through the
  ordinary approval cards with that environment's settings. They can rename, stop, restart, replace, prompt and delete
  sessions in the same environment (never their own session for delete or prompt, as before). Listings show only their
  own environment. Clone, template commands, host commands, creating anywhere else, and naming another host are refused
  with "Agents in sandbox and container sessions can only act within their own sandbox or container." An agent's Delete
  of another session in its environment removes only that session; it can never remove the environment.
- **Agents on ordinary hosts** can run `farhelm agent create --recipe <name>` (with `--container-host <host>` for a
  container), use a template whose target is a recipe, or clone a sandbox or container session. Each shows a new
  approval card that always asks, with Allow and Deny only, naming the recipe, where the environment will be created
  and, for Tensorlake, that it bills. Allow is the creation confirmation, so there is no second billing question. A page
  from an older release cannot show this card: while one waits, that page's approval list stops updating and keeps
  showing its last list, the same limitation you accepted earlier for the approval cards that carry a prompt and those
  that move a shortcut. Delete of a sandbox or container session from an agent outside it stays refused, as does any
  create aimed at an existing environment from anywhere but inside it.
- **Your Delete of a session in a sandbox or container** now asks the Farhelm inside the environment when the helm is
  connected to it. The confirmation states both outcomes (this session and its checkout go; if it was the last session,
  the sandbox or container and all its files go too) and is never skipped by "Skip session confirmations". When other
  sessions remain, only that session goes and the page says "The container stays: 2 other sessions are in it", with the
  count the inside Farhelm reported. When it was the last session, that Farhelm seals itself (it refuses every new
  session, Restart and terminal tab from then on, from any source, with or without a helm connected) and the helm
  destroys the environment. When the environment is suspended, paused, unreachable, running an older Farhelm, or gives
  no answer, nothing is decided: the page offers the old whole-environment Delete with a warning that every session in
  it is destroyed, including sessions Farhelm does not list, and that resuming or updating first lets Farhelm delete
  just this one. Bulk Delete sends one such request per marked session; only sessions of environments that were not
  connected when the dialog opened are grouped into warned whole-environment rows, and the dialog says which rows are
  which.
- **An environment being deleted** shows as "Deleting…" in Hosts and in the sidebar's attention area, and its sessions
  show a Deleting badge with their actions unavailable. If destroying it fails after its last session was deleted, it
  shows "Deleting failed" with the reason, and Retry deletes it again. The helm also finishes destroying any sealed
  environment it finds on connecting (after a lost reply, a failed destruction, or its own restart), without asking
  again (the landing monitor's choice when it adjusted your design: you had already confirmed that outcome, and a sealed
  environment otherwise keeps running and billing).

Delete sandbox and Delete container in Hosts are unchanged: they always destroy the whole environment.

## Things you should know

- **Older Farhelm inside environments.** The new Delete request is a protocol change (the stack moves the protocol from
  74 to 76; 75 is the new agent card and command). An environment whose Farhelm predates this cannot be asked, so its
  sessions always get the warned whole-environment Delete until it is updated. Ordinary remote hosts need updating again
  after this lands, as after any protocol change. Only pages from this release ask for the new Delete. A page from an
  older release still shows the old Delete for a sandbox or container session, with its old confirmation, and that still
  destroys the whole environment, as it did before this plan.
- **The deleted session's checkout is discarded, not moved to the checkout trash**, whether or not the environment stays
  (the landing monitor's choice in the Decisions; environment Deletes never archived before either). A checkout another
  session in the environment still uses is kept.
- **No notification when an agent is refused.** The plan said to keep an existing at-most-once notification for refused
  agent requests, but in the same 2026-11-09 decisions of the sandbox-sessions plan you had removed it (refusals there
  add no notification). I added none; the acceptance criterion ("at most one") is met by zero.
- **`farhelm spawn` still means "another session here".** Launching into a new environment is
  `farhelm agent create --recipe`, not a spawn flag. This narrows the TODO entry "`farhelm spawn` and CLI flags", as the
  plan proposed.
- **Agent launches into new environments cannot use an idempotency key** (`--key`): the record would live inside the
  environment the first attempt created, where a retry could not find it before creating a second environment. The
  command refuses the key.
- **How long the agent's command waits.** An agent's environment launch waits for the nine-minute approval window plus
  thirty minutes, counted from when the request reaches the helm. A launch still unfinished then is reported to the
  agent as an unknown outcome while it carries on; the session appears in listings once it exists. The agent
  instructions tell agents to allow forty minutes or run the command in the background.
- **Waits around the new Delete, and what you see when one runs out.** A second Delete for the same environment waits up
  to 60 seconds for the first (so two windows deleting its last two sessions get one "stays" and one destroy, in order);
  past that, the page says the environment is busy and to try again. The inside Farhelm gives up within 30 seconds if
  the session is busy (a running Restart, another operation) and answers "busy, nothing was deleted", which the page
  shows the same way. The helm gives the inside Farhelm 240 seconds to answer; no answer means nothing was decided and
  the page offers the warned Delete. The page itself waits up to nine minutes for the whole Delete including the
  destruction; past that it shows an error on the row and offers nothing automatically, since the outcome is unknown.
- **Which confirmation you get depends only on whether the helm is connected to the environment right now**, never on
  the provider. When the page has not yet learned, or failed to learn, whether the helm is connected to that
  environment, it shows the confirmation that asks the inside Farhelm; the helm decides again when it sends and, if it
  cannot ask, the page then offers the warned Delete. It never sends a whole-environment Delete you did not confirm in
  that warning.
- **The "stays" message can be briefly wrong in one case the landing monitor accepted when it adjusted your design
  (2026-11-12)**: two windows deleting an environment's last two sessions at once can show "stays: 1 other session" in
  one window just before the other window's Delete destroys it. It clears itself when a hosts refresh no longer lists
  the environment, and also when a later Delete in the same environment completes (replaced by the new count, or
  removed). A notice about the deleted session's checkout (that it could not be removed, say), if one was shown beside
  it, stays.
- **An environment sealed and then suspended before the helm read the seal** shows with no sessions until the helm can
  reach it again (a case the landing monitor accepted in that adjustment). Hosts' Delete sandbox or Delete container is
  the way out meanwhile.
- **The helm's record of a sealed environment lives in memory only.** If the helm restarts after a failed destruction,
  it learns the seal again from the environment's own session list on the next connection and finishes the destruction
  then. Until that connection, the environment shows in Hosts as an ordinary sandbox or container with no sessions, not
  as being deleted. A Tensorlake sandbox in that state bills at the full rate only until the helm reconnects or
  Tensorlake's 30-minute idle timeout suspends it; once suspended, it keeps billing for snapshot storage until you
  Resume or delete it. See the open question below.
- **A seal reported by an ordinary host is ignored, defensively.** The helm only ever asks sandboxes and containers to
  seal, so this needs a host whose own state says sealed without that request (a state directory copied from an
  environment, say). Such a host shows a failed refresh naming the seal, and the helm never destroys or forgets an
  ordinary host because of it.
- **Replace and Delete together.** If you delete a session while its Replace is in flight, either the Replace's new
  session was admitted first (then the Delete is not the last one, and the Replace's own removal of the old session
  counts as done when it finds it already gone), or the seal came first and the Replace says "Replace did not start: …
  is being deleted". Counting "already gone" as done is limited to sessions in environments.
- **Adding a Tensorlake sandbox that is being deleted** (setting up an existing sandbox as a host) is refused.
- **The Retry menu item keeps its usual description**, "Try the connection again", for a failed deletion too; the row's
  remedy line says "Retry to delete it again". I did not add a second description to the menu for this one case.
- **While a session's environment is being deleted**, pin, move and mark-read stay available (they are page-side
  preferences), and Rename stays as it is (it fails with the ordinary "host not connected" error). Bulk Stop and Delete
  leave such sessions out, give each one the reason on its row, and check again when the batch starts, so a dialog
  opened just before the environment started being deleted sends nothing to it.
- **A Delete answered "not found"** counts as done for environment sessions and removes only that session from the list:
  the page cannot tell whether an earlier attempt or this run's own seal removed it.
- **Tests against the local Podman** cover the whole new Delete with a real container: an agent inside creates a second
  session, the Delete while paused answers "nothing decided" and leaves the container paused, the first Delete keeps the
  container and reports one other session, and the last Delete seals and destroys it. That real-container scenario no
  longer exercises the warned whole-container Delete (its one container now ends through the seal); the browser tests
  cover that path against faked answers. Tensorlake is covered through the helm's provider stand-ins and browser fakes,
  never a real sandbox.
- **Two scope reassessments.** The new-environment launch (#3348) needed five review rounds; a fresh review of its scope
  kept the design but suggested four internal tidy-ups to that code (fewer repeated checks and one fewer code path),
  none of which changes what users see. I did not take them, to keep that PR from widening. The failed-deletion state
  (#3361) needed three rounds of fixes for ways to lose it (a Suspend, a reconnect, a cleanup failure). The reassessment
  replaced those individual checks with one rule: while the helm holds a seal record for an environment, Hosts and the
  sidebar show it as being deleted, whatever else happens to its connection. Later reviews found nothing more there.
- **Design consult.** The new request and protocol bump went through the plan's Opus consult first. It approved with
  thirteen changes, all adopted, among them: only pages from this release can ask for the new Delete; a pending spawn
  card of the deleted session is denied before the request, so it cannot hold the Delete up; the count and the seal are
  one database transaction; and one "being deleted" state is shared by every path that destroys a machine.

## Open questions and possible follow-ups

- **A sealed sandbox after a helm restart.** If destroying an environment fails and the helm then restarts, the helm
  forgets that the environment was sealed until it connects to it again. A running sandbox reconnects and is destroyed
  then. But if the helm is down for more than about half an hour, Tensorlake's idle timeout suspends the sandbox, and
  Farhelm never resumes one by itself. It then sits in Hosts as an ordinary empty sandbox, billing for snapshot storage,
  until you Resume it or use Delete sandbox. Options: (a) keep the helm's seal record across restarts, so it keeps
  showing as "Deleting failed" with Retry. That would be helm-side state about a session's end, which SPEC_impl.md
  forbids and your approval for this plan did not cover (it covered only the seal inside the environment), so it needs
  your exception. (b) Leave it. It takes a failed destruction followed by a helm restart that keeps the helm down for
  more than about half an hour; a shorter restart reconnects and finishes the destruction. I recommend (b), because a
  failed destruction followed by a helm restart is itself rare, and Delete sandbox in Hosts still removes it.
- **Real-container coverage of the warned whole-container Delete.** Adding it means a second recipe launch in the
  real-Podman scenario (a few more minutes per run). I recommend leaving it: the destruction itself is the same call
  Hosts' Delete container uses, and the browser tests pin that the page sends it only after the warning.
- **The four tidy-ups from #3348's scope review** (above): one shared check, before the card, that a recipe can run at
  the chosen place; the launch handing back its own record of the new session instead of the helm reading it again;
  reading templates once instead of twice; and one setting for "the agent chose where this goes". None changes behavior
  or fixes a known risk, so there is nothing to decide unless that code is touched again, when I recommend doing them.
- **Bulk Delete with an unreachable environment** is covered by unit tests of the dialog's planning, not a browser test:
  the browser test setup cannot show one sandbox as suspended and another as connected at the same time, and teaching it
  to would be new test infrastructure. I recommend leaving it unless bulk Delete changes again.

## The PRs

- [#3334](https://github.com/scode/farhelm/pull/3334/changes): SPEC.md and SPEC_impl.md for agent requests in
  environments, the new-environment card and CLI flags, and the new Delete and seal.
- [#3338](https://github.com/scode/farhelm/pull/3338/changes): agents in sandboxes and containers act within their own
  environment; docs; adds the feature's changelog entry.
- [#3348](https://github.com/scode/farhelm/pull/3348/changes): agents start sessions in new sandboxes and containers
  (`--recipe`, `--container-host`, recipe templates, Clone), behind the always-asking card; extends the changelog entry.
- [#3354](https://github.com/scode/farhelm/pull/3354/changes): real-container and browser tests for those, and the
  TODO.md removals.
- [#3359](https://github.com/scode/farhelm/pull/3359/changes): the inside Farhelm's delete-or-seal request and the seal.
- [#3361](https://github.com/scode/farhelm/pull/3361/changes): the helm's environment Delete, finishing sealed
  environments, the Deleting state and Retry.
- [#3364](https://github.com/scode/farhelm/pull/3364/changes): the page's environment Delete (both confirmations, the
  "stays" message, bulk, being-deleted sessions), docs pages, and the browser and real-container tests; rewrites the
  changelog entry for the new Delete.

Docs pages changed across the stack: Approve what agents do (new sections on agents in sandboxes and containers and on
new sandboxes and containers), Tensorlake sandboxes (a new section on deleting a session in its own sandbox), Container
sessions, Supported agents, Stop, restart, and resume, Start a session, and Launch templates. The changelog has one
`added` entry for the whole feature, written in #3338, extended in #3348 and rewritten in #3364; #3359 adds a `none`
fragment for the seal (context for the curator) and #3361 a `changed` one for the Deleting state in Hosts.

## Checks run, reused and skipped

Runtime tests ran through the test-run recorder: Rust in its nextest mode (four slots, no retries, the pinned tmux where
the selection needs it), browser specs in its generic mode on Chromium and WebKit after a fresh `cargo build` and web
release build of the revision tested. Runs that failed are kept; every one was a mistake in a test written in the same
session or an environment problem, and was fixed and re-run.

The stack was rebased twice onto newer main, last onto the change that stops a quick launch hanging after a sandbox host
wakes (the wake-launch-stuck plan). That change touched only the page's session launcher, its browser specs, SPEC.md,
SPEC_impl.md and docs: nothing in the helm, the supervisor, the CLI, the protocol or the real-container test. Its one
overlap with this stack is the launcher file where #3364 changes one sentence of the "Create a sandbox" confirmation,
and no spec on main checks that sentence. So helm, supervisor, CLI and real-container evidence from before the last
rebase is reused, and the page's tests were re-run on the rebased stack.

Run on the final stack (#3364 as pushed, on current main):

- `cargo nextest run -p farhelm-ui --lib`: 997 passed (`0743a17f-088c-49a3-9bfc-a2746f5d31aa`). On the rebased stack
  before the last two review fixes, which added two tests: 995 passed (`adf2cefe-f7ce-40f7-8d11-87dd554c6089`).
- `npx playwright test` for the Tensorlake sandbox sessions, container sessions and bulk session actions spec files,
  both engines: 88 passed (`c3c3b9dd-5c00-4fc9-af07-6605dbd281c4`).
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --target wasm32-unknown-unknown`,
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python -B scripts/check-test-sleeps.py` (no unexplained delays) and the website build: all clean. The workspace
  clippy ran before the last two review fixes, which changed only the page's code; the page's own clippy ran after them.

Reused, with the revision each covered and why it still applies:

- The real-container scenario, `cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'` with the
  pinned base image: passed with the new Delete steps on #3364 before its review fixes
  (`62c2017c-ccdc-4453-8991-9a5745d11051`, about five minutes, no `SKIPPED`), and with the agent scenario on #3354
  (`7b22a2b9-6e3e-4ea5-9b41-c5eac8ae72a3`, `e051ae61-53c4-4b4c-b6a1-6dd27950cd0e`). Since then only the page's code
  changed. I pulled the pinned base image for these runs and removed it afterwards; the test's own recipe images and
  containers were cleaned up by the test.
- Supervisor and protocol libraries: 2061 passed (`1c668e1c-9498-44cf-9103-d2688a8ce733`), #3359 as pushed; #3361 and
  #3364 do not change the supervisor or the protocol.
- Helm and page libraries: 2747 passed (`5c9ded80-bf5c-4fe2-b243-c20ec686da9d`), #3361 before its last one-line review
  fix, after which the environment Delete, sandbox setup and removal tests passed again, 29 of 29
  (`03386ccd-351c-46aa-b90f-589fb0e917ce`); #3364 does not change the helm.
- Earlier PRs. #3359 to #3364 do not change the CLI, the handling of agents' requests or the approval cards; #3359 did
  change how the supervisor ends a deleted session, which an agent's Delete also uses, and that is covered by the
  supervisor library run above and by the real-container scenario, where an agent deletes a session. The earlier runs:
  the helm library after #3348's last fix, 1766 passed (`aa3f1523-222b-48c9-9857-0138eaacacaf`); the CLI's own tests on
  #3348, 199 passed (`7aaa9734-468d-4de9-a20b-055424a59196`); the end-to-end tests of agents' requests through the
  supervisor and the CLI on #3348, where the first run failed one test whose expected behavior this plan changed and,
  after that test was updated and renamed, the second run passed it, 77 passed in all
  (`c2623fa7-d7e8-4f2d-b718-f949794e5512`, `218ab381-084a-4ff4-a85a-548911f36064`); and the new-environment card's
  browser layout audit on #3354, 68 passed (`d2edfe4c-9208-4c4d-a0bf-52ea22a1861b`).
- The page's specs at intermediate points of #3364: 86 passed twice (`758d3366-0614-4f58-a5cb-9f0743480a6f`,
  `6dab3224-2b6d-49dc-a09b-e8072b0d1bfe`), superseded by the final run above.

Skipped: the rest of the browser suite, the workspace Rust battery and the old-terminal browser set. The Delete changes
reach the page only through the session list, the session header and the bulk dialog, which the three spec files cover;
the helm and supervisor changes are covered by their libraries and the real-container scenario; nothing here touches
terminals, the launcher's flow, installers or the desktop app. No real Tensorlake sandbox was used: the provider is
exercised through the helm's stand-ins and the browser fakes, as the plan asked.

## Review gate

Each PR had a fresh-context gpt-6.1-sol review at high effort, as the plan requires, repeated after fixes until a round
came back clean, except for three fixes applied without another round (#3334's later wording amendments, #3338's one
docs fix, #3361's last one-line fix), each noted below. The new protocol request went through the plan's Opus consult
before it was built (above).

- #3334 (spec): two rounds; two findings fixed (environment hosts start with YOLO launches allowed, so agent launches
  pass the YOLO rule by construction; the agent's wait is counted from the request, not from Allow). Its later wording
  amendments were read as the contract by the reviewers of #3348 and #3359 to #3364 rather than reviewed as #3334 again.
- #3338: one round; its one finding (a docs page said "everything else is refused" though opening a URL is allowed) was
  fixed with the reviewer's own wording, not re-reviewed.
- #3348: five rounds and a scope reassessment; fixed among others a crash when an approved launch reported its new
  session (it named the session's host twice), the new environment's folder trust decided only after Allow instead of
  before the card was shown, agent launches recorded in launch history as the user's, templates that mix placements,
  cards for recipes that cannot run there, Clone of sessions whose launch today's settings would refuse, and a recheck
  of the Clone source after Allow.
- #3354: two rounds; two test fixes (a refusal named an unregistered host; a sibling deleted before it was proven
  running).
- #3359: four rounds; fixed the bounded waits, an immediate "sealed" answer during cleanup, the seal surviving a failed
  or oversized session list, checkout-trash problems reported, and the discarded checkout recorded in the same
  transaction as the delete. One reviewer preference not followed: older-database test fixtures are handled by letting
  the upgrade add the new column only when missing, instead of editing sixteen fixtures; no real database at the old
  version has the column.
- #3361: five rounds and a scope reassessment (the ways to lose the failed-deletion state), which led to the
  shown-while-sealed rule above. The last round's one small fix (a Retry kept showing the old failure while it ran) was
  the reviewer's own one-line suggestion and was not re-reviewed.
- #3364: four rounds; fixed a Delete answer without the remaining-session count making the page drop the environment's
  other sessions from the list, bulk actions reaching sessions being deleted, and the "stays" message outliving the
  environment (twice). The last round found nothing.

### Landing

#### What landed with it

Nothing else landed in this round, and main gained only plan-queue and TODO.md changes since the stack was based, so
nothing on main interacts with it. Four independent reviewers read the whole stack in parts (the spec and the seal, the
helm's Delete, the page's Delete, and agents in and launching environments) against the plan's Decisions, your
2026-11-12 Delete design above all. They found no design problem and no way for an agent inside a sandbox or container
to reach outside it: the helm decides an agent's reach from the connection its request came in on, never from what the
request says. They did find about a dozen smaller defects, two of them real bugs, all fixed below before anything
merged; I read every fix, and the checks after them are listed at the end. The design holds as built:

- the environment's Farhelm counts its sessions and seals in one step that no create can slip into, counting sessions in
  any state, including one still starting or restarting;
- once sealed it refuses every way of starting something: a new session from any source, a retry of an earlier create,
  resuming a checkout launch whose reply was lost (the helm's recovery path for those), Restart, Restart with, and
  opening a terminal tab; the seal survives a restart of that Farhelm;
- the helm never decides "last session" itself, never destroys anything on an unanswered request, and picks which
  confirmation to show only from whether it is connected to that environment's Farhelm.

The tmux removal plan being built now, tmux-end-old-terminals, makes its own protocol change and rewrites Restart and
tab opening. It has to take protocol version 77, since this stack uses 75 and 76, and keep the seal checks this stack
put into those paths. I will check both when that plan comes to land.

#### Fixes made while landing

Each fix went into the PR whose code it changes; together about 600 added lines, most of them the new tests.

- **A guarded Delete could destroy an environment.** The page's Delete request to the helm can carry conditions ("only
  if nothing is running", "only if the agent has ended"), which the helm ignored for the new environment Delete. The
  page never sends them for it and nothing else Farhelm ships calls this Delete, so only a hand-written API call could
  hit it, but such a call would have deleted a running session and, for the last one, sealed and destroyed the
  environment. The helm now refuses it, as the whole-environment Delete already did, before the environment's Farhelm is
  asked anything (#3361).
- **An agent's new-environment launch could be cut off halfway.** It ran on the asking session's own connection, so if
  that connection dropped right after Allow, the launch could stop between registering the new environment and starting
  to build it, leaving an environment entry nothing ever builds or removes, or a host shown as setting up until the helm
  restarts. It now runs as the helm's own work, as your own launches already do; a dropped connection only loses the
  agent's reply (#3348).
- **The "stays" message outlived the environment.** "The sandbox stays: 1 other session is in it" went away only when
  Hosts stopped listing the environment, so it showed through the whole destruction after another Delete sealed it, and
  indefinitely beside "Deleting failed" when destruction failed. It now goes as soon as Hosts shows the environment as
  being deleted (#3364).
- **Specs still stating the old rule.** SPEC.md's "Skip session confirmations" passage, its bulk Delete dialog sentence
  and its Durability paragraph still described Delete as always destroying the whole environment, and SPEC_impl.md
  described the sealed refusal wrongly. All four now match the design (#3334).
- **Three missing tests.** Your design asked for tests that a retried create and a resumed checkout launch are refused
  once sealed; neither existed. Both are added, and a run with the seal checks taken out failed both, so they catch the
  bug (#3359). Separately, #3338 rewrote the test that checked an agent inside an environment cannot prompt a session
  elsewhere so that it no longer reached that check (agent prompts are switched off in this release, and the rewritten
  test kept them off); a new test switches them on and checks it, so turning prompts back on cannot open that hole
  (#3338).
- Smaller: the new-environment card's wording for where the container or sandbox goes is now defined with each
  provider's other wording, so a new provider has to choose its own (#3348); comments that claimed a failed destruction
  keeps the host's connection held, and that a sealed host refuses Suspend and Resume, now say what the code does
  (#3361); four code comments the seal PR had separated from the code they describe are back in place (#3359); and the
  docs pages now say the deleted session's checkout is kept when another session still uses it, as the confirmation does
  (#3364).

#### What the report says that is not quite right

- "The 'stays' message can be briefly wrong in one case": it now also goes when Hosts shows the environment as being
  deleted, not only when Hosts stops listing it, so it no longer lingers through a destruction or after a failed one.
- The changelog fragment of #3359 (an internal note for the release curator, not a user-facing entry) says nothing uses
  the seal yet. That was true only of #3359 on its own. Nothing needs doing: a `none` fragment adds no changelog entry.

#### Things you should know that the report does not say

- **A new-environment launch holds up the asking session.** While an agent's launch into a new sandbox or container runs
  (up to the nine-minute approval wait plus thirty minutes), Farhelm keeps that agent's session busy: your Delete or
  Replace of that session waits until the launch ends, with no limit of its own, and the agent's other farhelm commands
  are refused meanwhile. The agent instructions tell agents to run the launch in the background, which this undercuts.
- **A process inside an environment can probably make Farhelm destroy that environment.** The seal lives in the
  environment's own Farhelm database, which a process running there can most likely write, and the helm finishes
  destroying any environment it sees sealed without asking (the landing monitor's choice when it adjusted your design).
  So an agent inside could have its own sandbox or container destroyed, snapshots included, without your Delete. It can
  already delete every file in there, but the design says only you delete an environment. Nobody checked the file
  permissions inside environments.
- **After a destruction fails, Suspend, Resume and Update are not refused.** The helm answers them as busy only while a
  destruction runs. Once one has failed, a hand-written API call can still suspend, resume or update that environment;
  the page offers none of them for an environment being deleted. Either way the environment keeps showing as "Deleting
  failed" and Retry still destroys it, whatever state it is in, so nothing is lost.

#### Possible follow-ups

- **Answer the agent at Allow** for new-environment launches, so the asking session is not held up for the whole build.
  I recommend doing it: the hold-up surprises both you (a Delete that waits) and the agent (refused commands).
- **Seals the helm did not ask for.** Either keep the environment's own processes from writing the seal, or have the
  helm ask you before finishing a seal it did not request. I recommend the second, which is simpler and covers any way a
  seal could appear: the helm knows which Deletes it sent.
- **Agent commands refused inside environments are listed one by one**, so a new agent command would be allowed
  everywhere until someone adds it to that list. I recommend making every agent command declare its reach, so a new one
  cannot be forgotten; it is a refactor with no visible change.
- **Two stale messages**: the launcher's note about managed checkouts for new containers and sandboxes still reads as if
  Delete removes the environment, and a refusal still says recipe templates can only be used in Farhelm's window, while
  agents can now launch from them. Worth fixing when that code is next touched.
- **The real-container test's weak spots**: it checks that an agent inside sees no other host only by the absence of the
  text "this machine", and it launches with `farhelm agent create` where the acceptance criterion says agents can
  "spawn" sessions in their own environment. I did not check whether `farhelm spawn` takes a path the test misses;
  tightening both checks is a small follow-up.

#### Checks

Browser runs were on Tensorlake sandboxes at your request; the sandboxes run Node 24.16 where the development machine
runs 24.20, which these tests do not depend on, and each shard is recorded in the recorder's mode for arbitrary
commands. Rust tests ran locally through the recorder's nextest mode (four slots, no retries, the pinned tmux).

On the stack as delivered, restacked onto main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-ui --target wasm32-unknown-unknown` passed.
- `cargo nextest run --workspace --exclude farhelm-desktop -E 'not test(/^podman_environments::/)'`: 6,140 passed (run
  `03e72af3-1c2a-4f04-b72d-99e2ec0a7cad`), the supervisor's end-to-end suite included, which had not run since the seal
  checks were added at every create, Restart and tab entry point.
- The whole ordinary browser suite on both engines, from a fresh build, on ten sandboxes: 3,315 passed, 3 failed, and 14
  skipped by the specs' own conditions (engine-specific cases, and the real-agent spec, which needs installed agents)
  (runs `9bda7618-fc29-4b84-a080-858ad2661419`, `f8bc84c0-0901-45b9-9515-c826b2dd2a0e`,
  `e560721e-56e0-433c-a547-d960a67fb67b`, `85997c5b-1b77-4d66-b091-8d1bc85aa15b`,
  `d6c70ec7-626e-49f2-a38a-c65606cdf02b`, `8d26b6d6-e4ac-4e15-8e0c-4e4e864f556d`,
  `82fee637-fd9e-4897-afd1-f7fb7d16cd3e`, `2dc5f83a-31b7-452d-a677-4b0c53e06de4`, `4b07bc95-d5a5-410a-93fd-f9916f44a903`
  and `67420e2b-7e76-47f0-b5b6-3d0e835d908b`). The three failures are open, recorded flakes in areas this plan does not
  touch, each on Chromium: the trash dialog's reachability at width 1000, a provisioning progress test ("incarnation
  churn keeps a followed run"), and a quick launch test (the page body never seen focused after a blur, as FLAKES.md
  records). All three passed in the run below.

After the landing fixes, rebased onto main again (plan and TODO.md changes only):

- The same lints, test-sleep check, changelog and agents-md checks, workspace and binary clippy, desktop and wasm
  compiles: passed.
- The specs for approval cards, bulk session actions, container and sandbox sessions, Tensorlake hosts, the hosts panel
  and retired container sessions on both engines, and the three flaky specs above on Chromium, from a fresh build: 342
  passed, none failed or skipped (runs `802eb609-ad1e-458b-a1c0-f666f0626c81`, `3df93d81-a5b1-44af-9be3-578e5804b9ce`,
  `e43d301a-07f7-49f8-8f66-26f5af0cbdd3`, `795a5fc6-cd9a-4cf0-8fad-0e560ff41bff`,
  `35da944b-5e04-4884-a5d2-8b4da687380b`, `55b30b39-79b8-4b9b-b04e-83857f82d6da`,
  `75e2b85d-1782-490b-ad32-dc548882d90a`, `1afd1765-66b4-47f5-ba21-a95f249fb741`, `9f510859-6849-4398-86ad-93d7da6dae02`
  and `12f0309e-2aa2-45d5-bc27-796fe7cbe211`).
- `cargo nextest run -p farhelm-helm -p farhelm-supervisor -p farhelm-proto -p farhelm-ui --lib`: 4,834 passed, 1 failed
  (run `0360b6db-c2ed-4d6c-8ce1-a48ffcd25b6f`). The failure, the helm's
  `provisioning::tests::a_plan_made_while_running_does_not_run_once_suspended`, hit its five-second wait for a
  provisioning run, most likely because the four crates' tests were sharing the machine; this stack does not change that
  code, the test passed in the workspace run above, and it passed ten times alone (runs
  `0a043bbe-2c1d-4795-b127-da306a7c641c`, `9efe7915-3ba7-4b1b-a63c-7355616beddc`,
  `c1b4d0fb-23f8-4677-8d02-c666cf599ac8`, `29158f2e-3f07-47fc-8a65-9ee883df993b`,
  `02f47423-2e9c-45ea-a13a-662bf10e1365`, `e3b3a21b-4aed-4bc2-b042-c1e81956c82d`,
  `52ccb4e8-c345-4a85-9551-cd7d69434ed7`, `731f9d80-b845-4f5e-926a-11bac0b2736b`, `9e8d6627-feb0-4aaf-b4ac-ee5851195737`
  and `385b0942-e354-417c-a8cb-1741999d0bc4`), each in under a fifth of a second. Its failed run is kept; one failure
  under load is not enough to record it in FLAKES.md, so it is not recorded there.
- The landing fixes' own tests, run while they were written: the supervisor's seal tests with the schema, seal-count and
  checkout tests, 21 passed (`6beefd04-d584-4965-bd05-18e94182609d`); the helm's agent request and session tests, 282
  passed (`8b559280-c4f8-4490-bf3f-017354356df4`); the page's list, approval, provider, hosts and sandbox session tests,
  167 passed (`a92ea0ee-c0e0-44a8-9078-bb1742bb0764`). A run with the seal checks deliberately removed failed both new
  seal tests (`c2fe7fdb-b4ad-468c-ace5-f0dbea9ae3c1`), showing they catch the bug.

Reused: the real-container scenario (`cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'`),
which the executor ran on #3364 before its review fixes (`62c2017c-ccdc-4453-8991-9a5745d11051`) and with the agent
scenario on #3354 (`7b22a2b9-6e3e-4ea5-9b41-c5eac8ae72a3`, `e051ae61-53c4-4b4c-b6a1-6dd27950cd0e`). Since then only the
page's code, the helm's refusal of a guarded Delete and how it runs an agent's new-environment launch changed; the
scenario sends no guarded Delete and launches no new environment for an agent.

After the merge, on main (the same tree outside the plan queue), because the landing fixes changed how the helm runs an
agent's new-environment launch and four docs pages:

- The end-to-end tests of agents' requests through a real supervisor and the CLI, with the CLI's own tests
  (`cargo nextest run -p farhelm --test e2e --bin farhelm -E 'test(/^agent_relay::/) | binary(farhelm)'`, then
  `-E 'test(/^agent_listing_real_stack::/) | test(/^prompt_running_agent::/)'`): 209 and 8 passed (runs
  `06186d40-38f6-4d53-9bc7-4348bd29fb70` and `a2ba1c3c-6f2c-41b1-ab43-9ebd8f369007`).
- The website build (`bun install --frozen-lockfile && bun run build` in `website/`): passed.

Skipped: the old-terminal browser set and the doctests; nothing here touches old terminals or documentation examples.
