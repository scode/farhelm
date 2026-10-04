### What this was about

Sessions could be started four ways: choices in the New dialog, a built-in profile, a user profile, or a typed command
line. Each way decided on its own whether a launch was YOLO, how to resume it and what Restart did, and Farhelm read
command lines to guess what they ran. That guessing produced a run of bugs: a profile's resume command could turn off
approvals without the sensitive-host YOLO confirmation noticing, and the resume command Farhelm built by appending the
conversation id argument to a typed command could restart into the wrong conversation. You decided (2026-10-03) to
remove profiles entirely and replace the four ways with two launch kinds and named launch templates; the spec change is
the first PR of this stack.

What the eight PRs now do, in product terms:

- **Two launch kinds.** An agent launch is an agent type you pick (Claude, Codex, …) with its choices; Farhelm composes
  its start and resume commands. A command launch is a command you write, with a required YOLO answer that Farhelm
  believes and never checks. As you decided, that means a command answered "not YOLO" that does turn approvals off is
  not asked about on a host that asks before YOLO launches. A command launch may declare the agent type it runs; it then
  marks with `{farhelm_args}` where Farhelm adds the arguments that turn on that agent's conversation tracking and the
  instructions pointer for agents, and gets that agent's status reading. It may also opt into Resume with a resume
  command containing `{conversation}`. A command with no declared agent type gets only generic running/idle status, no
  conversation tracking and no Restart. Nothing reads a new session's command line any more to decide YOLO, agent type
  or resume (one exception for OMP is below). The New dialog shows the two kinds as an **agent** and a **command** tab.
- **Restart only resumes.** There is no fresh restart and no "run this command" fallback. A session whose conversation
  Farhelm cannot resume shows Restart greyed out with the reason; Replace starts it over. Cursor, Muse and OpenCode
  sessions therefore can no longer be restarted at all, since Farhelm cannot resume their conversations.
- **Restart with** now also works for command launches: it lets you change the command, YOLO answer, declared agent type
  and resume command while resuming the same conversation.
- **Profiles are gone**, with no conversion: the Profiles panel, the profile picker, built-in profiles, the agent CLI's
  profile selectors, and the helm's stored profiles and remembered default profile, which the upgrade deletes. Commands
  someone wrote into a user profile are not kept anywhere; they would have to be re-created as templates.
- **Launch templates.** A **templates** button beside New opens a dialog to create, edit and delete named sets of
  launcher choices; `tl:name` in the launcher's search applies one exactly as making those choices by hand, all or
  nothing, stacking in order. Sessions never record which templates made them.
- **The agent CLI takes the launcher's fields.** `farhelm agent create` and `farhelm spawn` take `--agent`, `--model`,
  `--effort`, `--permissions`, `--trust`, `--command` with `--yolo`/`--no-yolo`, `--resume-command`, and repeatable
  `--template`; templates apply first and the flags then edit the result, and a field nothing set is refused naming the
  flag rather than filled from your GUI defaults. `farhelm agent templates [--json]` lists templates without command
  text. A spawn with launch flags goes through the helm; `spawn --inherit-agent` still works with no helm.
- **Upgrade.** Sessions created from New-dialog choices become agent launches and behave as before. Sessions from a
  profile or a typed command stay "legacy": they keep running and Restart still works when it can resume, but Replace,
  Restart with, `farhelm agent clone` and `spawn --inherit-agent` refuse them with a remedy (Replace with or Clone opens
  the launcher with their command filled in), and their permission mark is a question mark. The wire protocol between
  helm and supervisors moves to version 40, so every host must be updated along with the helm, as for any protocol
  change; hosts left behind show "needs update". Downgrading is not supported: an older release cannot read the upgraded
  databases and refuses to open them. The upgrade is tested by the migration tests, which start from database fixtures
  in each earlier schema (including sessions created from New-dialog choices, from profiles and from typed commands) and
  check the result; it was not run against a real database copied from a previous install.
- **Docs website** pages that described profiles, the old "other / command" button, fresh restarts and appended hook
  arguments are rewritten; the three TODO.md entries this plan covers are removed.

### Things you should know

- **Retired CLI spellings are refused, not aliased**, each with a message naming the replacement:
  `farhelm agent
  profiles` (names `farhelm agent templates`),
  `farhelm agent create --profile`/`--profile-id`/`--invocation`, `farhelm
  spawn --profile-id`, and
  `farhelm agent restart --mode`. `farhelm spawn --agent` now means the agent type, not a profile name. Agents learn
  these commands from `farhelm agent instructions`, so the ones that hit a refusal are agents in sessions started before
  the upgrade that still hold the old instructions in their conversation; the refusal tells them what to use. The
  `--json` envelope of the agent listings is at schema version 6 when this lands: a script that reads it sees
  `restart_offer` values that say why a session cannot be restarted (instead of the removed fresh and fallback offers),
  an `agent` field that is an agent type's word or `custom` (never a profile name), and the new templates listing.
- **A PR in this stack had wrongly deleted spec text, found while rebasing onto current main.** The remove-profiles PR
  had deleted three unrelated implementation-spec paragraphs (the hosts panel, the settings dialog and the host actions
  menu) along with the profiles text. They are restored from main's own current text, unchanged. Earlier in the run the
  same PR was found to have removed a block of unrelated styling (stale-host notice, interrupted-session card, Replace
  confirmation), which was restored in that PR and covered by the full browser run.
- **An agent's keyed retry is bound to its first resolution for 30 days.** The spec says a retry with the same
  idempotency key returns the first session even if a template was edited since, and that keys live as long as their
  session. The supervisor still keeps each key as long as its session, as before. What is bounded is the helm's record
  of what the first attempt's templates and flags resolved to, which a retry needs only if a template changed in
  between: it is kept for 30 days and at most 256 per asking session, so storage cannot grow without limit and one busy
  agent cannot evict another's records. A retry past that is resolved afresh: it still returns the session if no
  template it names changed, and is refused as a key conflict if one did. The changelog says "for up to 30 days".
- **An explicit `--host` wins over a template's host on the CLI, even when the template's host no longer exists**, so a
  template pinned to a reinstalled host stays usable from the CLI. The launcher's `tl:` still refuses such a template;
  you can fix it in the Templates dialog, agents cannot.
- **A spawn with launch flags keeps a spawn's rules**: its parent must be the asking session, and its idempotency key
  lasts only as long as the child, exactly like `spawn --inherit-agent`.
- **Two Codex-internal markers are still filled in every launch.** Farhelm composes a Codex agent launch's workspace
  trust choice using the internal text `{codex:trusted-cwd}` or `{codex:untrusted-cwd}`, which it replaces with the
  session's directory. That replacement still runs on command launches too, so a command that contains that exact text
  gets it replaced. Nobody would type it by accident, and it does nothing else, but the spec names only `{cwd}`,
  `{conversation}` and `{farhelm_args}` for command launches.
- **The two places where old command-line reading remains.** Legacy sessions (from a profile or a typed command) keep
  the previous release's behavior: Farhelm still appends its arguments to their command, and still leaves them off when
  the command already passes its own Claude settings, configures Codex hooks itself, or contains a `--` separator. And
  for every OMP session, Farhelm still checks the shape of the running OMP process when a conversation report arrives,
  because that check is how it proves the report came from the session's own OMP rather than a child process; it decides
  nothing about the launch.
- **Custom commands for Goose must name `session` themselves** (`goose session {farhelm_args}`): Farhelm no longer
  inserts it, because it no longer reads the command. A command that leaves it out starts Goose without an interactive
  session, as running that command by hand would. Legacy Goose sessions keep the old behavior. The Goose docs page says
  so.
- **Pages left as stubs.** Launch templates got a new stub page and the custom commands page stays a stub, because a
  written page needs annotated launcher screenshots, which are captured and published in a session with you.
- **Wording decision in the spec PR:** when it was rebased earlier in the run, main's then-new sentences in SPEC.md
  about how conversation reports are accepted said "every integrated kind"; they now say "agent type", the spec's term.
  No behavior change.

### Open questions and possible follow-ups

- Do you want any retired CLI spelling kept as an alias? I recommend keeping the refusals: each names its replacement,
  and an alias for `--invocation` cannot work anyway, because a command launch needs a YOLO answer the old flag never
  gave.
- The keyed-retry bound (30 days, 256 per asking session): either write the bound into SPEC.md, or make the helm keep
  each record until its session is deleted, which needs the helm to clean up records for sessions on any host, including
  hosts that are offline when the session goes. I recommend stating the bound in SPEC.md.
- The Codex markers in command launches: either note them in the spec, or stop filling them in command launches, which
  needs the launch kind passed into the supervisor's directory filling. I recommend a one-line note in SPEC_impl.md
  only, since the behavior is unreachable in practice.
- Please refresh the docs screenshots: the published Start a session screenshots predate the agent/command tabs, and the
  two stub pages need screenshots to be written.
- TODO.md entries that still mention profiles, which I left for you, with what I think each needs: the Muse entry
  ("built-in `muse` and `muse-yolo` profiles") is partly obsolete, since built-in profiles are gone; "Close the
  cross-host execution hole" names the deleted profile-resolving relay and "trusted profiles", which would now be
  "trusted templates"; "Split `HelmStore` by concern" and the UI create-form refactor name deleted profile code and need
  that part dropped; "Custom hover tooltips" mentions "the profile chip", which no longer exists.
- The browser test `shell-scroll.spec.ts` fails in its cleanup on both engines: it stops its sessions all at once and is
  refused by the host's busy limit. Its cause is the busy limit on main, not this stack, so it presumably fails on main
  too (not rerun there). I recommend a separate small fix outside this plan, having the test stop sessions a few at a
  time.
- The CLI cannot clear a command launch's declared agent type that a template set; it can reset every other choice, and
  drop a resume command with `--no-resume-command`. A `--no-agent` flag would close it; I would wait until someone needs
  it.

### PRs

All eight are open drafts, stacked in this order, each based on the one before; none is merged or marked ready.

- #1537 `docs: specify launch kinds and templates to replace agent profiles` — the spec.
- #1559 `feat!: restart only resumes the conversation`.
- #1560 `feat!: remove agent profiles`.
- #1563 `feat!: launch kinds` — the two launch kinds end to end and the upgrade of existing sessions.
- #1571 `feat: launch-kind tabs in the launcher` — and Restart with for command launches.
- #1572 `feat: launch templates` — storage, the Templates dialog, `tl:`.
- #1576 `feat!: agent launches from the agent CLI` — the launch flags, `--template`, `farhelm agent templates`.
- #1577 `docs: describe launch kinds, templates and resume-only restart` — the docs website and TODO removals.

### Checks

Run now, on the whole stack rebased onto current main (which had gained remote uninstall, in-app feedback and the
add-host dialog trim since the stack began):

- the full workspace Rust suite, including the end-to-end tests: 3255 passed, none failed (run `301b9565`);
- a browser selection on both engines, chosen for where main's new help menu and host menu meet this stack's launcher
  and templates button: the sidebar, window chrome, templates, launcher tabs, feedback, host setup, launcher search,
  Restart with and multi-host terminal specs, 470 passed (run `2eb04cb8`);
- both JavaScript harnesses (190 and 10 passed), the doctests, both Clippy configurations, formatting, the Markdown
  formatter, the test-delay check, the changelog lint, the desktop build check and the docs website build.

Reused from earlier in the run: the full browser suite on both engines at the launch-kinds PR, where 1197 passed and 21
failed; this stack caused all but the `shell-scroll` cleanup failures above, those were fixed, and the failing specs
other than `shell-scroll` were rerun with no failures (482 passed). Each later PR also ran its own targeted Rust,
browser and lint checks. The rebase is covered by the runs above.

Skipped: the release-only gates (the pinned-tmux shutdown subset, the desktop smoke, the CentOS provisioning test, the
installer and uninstaller suites), because nothing in this stack touches installing, provisioning, the desktop shell or
tmux handling; the desktop asset parity check, because the stack adds no asset files (the Templates dialog's styling is
in the existing stylesheet); the desktop-feature Rust tests and doctests, because the stack does not touch the desktop
app's persistence or bridge; and the full browser suite at the tip, because the selection above covers the places where
main and this stack meet, and the earlier full run covers the rest of this stack's browser behavior.

### Review gate

Every PR went through the plan's fresh-context review (Opus, high effort) and a cold read of its commit message. All
findings were fixed except a few declined with a recorded reason, none with a user-visible effect: refusal messages keep
the protocol spelling of agent types (`open_code`), since that is what templates and the CLI take; two internal code
structures were kept as they are; and there is no separate CLI test for a flag spawn without a helm, since the relay's
existing "no helm is attached" refusal is passed through unchanged. The most consequential findings, all fixed: a spawn
with launch flags could record any session as its parent and kept its idempotency key forever; an agent retrying a
create the target host had refused would have got a key conflict instead of the original refusal; and the docs still
said a custom command that turns off approvals triggers the YOLO confirmation. The rebase fixes made after the reviews
were mechanical: restored text is main's own, and the conflicts kept both sides' additions.
