# Hover help: fast, themed tooltips on every icon and clickable control

Written against main at 7444371d on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `plans/queue/launch-representation.md` (its `INDEX.md` line says so). That plan removes the
profiles UI and rewrites the new-session form, Restart with, the YOLO confirmation and the sidebar's permission mark, so
names, files and buttons below come from main at 7444371d and some will have moved or gone. Read what it landed before
taking inventory.

## The goal

Every icon that carries meaning and every clickable control in the web UI and the desktop app shows a short hover text
saying what it is or does, and that text appears quickly: about 300 ms after the pointer comes to rest, not the
browser's own delay of a second or more. Today only about a dozen of roughly 110 buttons have any hover text, and the
hover texts that do exist (the sidebar's agent and permission marks, the status dot, the activity time, the folder line,
the restart buttons, the text-size buttons) use the browser's native `title` tooltip, whose delay is owned by the
browser engine and cannot be shortened by any attribute, CSS or script. WebKit, which the macOS desktop app embeds, also
ignores the macOS tooltip-delay default. The maintainer did not even notice the existing ones because they take so long
to appear.

So the work has two halves: a tooltip of Farhelm's own that replaces native `title` tooltips everywhere, and a coverage
pass that gives every clickable control and meaningful icon a hover text, guarded by a browser test.

Acceptance criteria:

- The tooltip, as seen by a user:
  - It appears 300 ms after the pointer rests on a control or icon that has hover text. While a tooltip is showing (or
    within a short grace period after one hid), moving onto another such element shows that element's tooltip at once,
    with no new delay.
  - It also appears when a control gains keyboard focus through keyboard navigation (`:focus-visible` semantics), in the
    same place.
  - It disappears when the pointer leaves the element, on any pointer press, on Escape, when the element loses focus
    (for a focus-shown tooltip), when the window loses focus, when the element leaves the DOM, and on a scroll that
    moves the element (the document, or a container holding it). Scrolls elsewhere, such as the terminal scrolling under
    live agent output while a header button is hovered, do not hide it. Detect a removed element by checking
    `isConnected` on the next pointer or focus event (or a short timer while showing), not with a DOM-change observer.
  - Touch input never shows it.
  - It looks like a small box with rounded corners holding the text in the app's UI font, one size smaller than body
    text, with the same background, border and soft shadow as the session row's `⋯` menu popover, in both light and dark
    themes (reuse the row menu panel's existing colors and shadow in `app.css` rather than new tokens). No arrow. Text
    wraps at roughly 280 px wide, and a maximum height clips very long text: session titles have no length limit, and
    the native tooltips being replaced were truncated by the platform. It is not interactive and never takes pointer
    events.
  - Placement: directly ABOVE the element, about 6 px away, horizontally centered on it, then shifted sideways as needed
    to stay fully inside the window. Only when there is not enough room above does it go BELOW, with a much larger gap
    of about 28 px. Rationale (the maintainer agreed): a web page cannot know the cursor's size, standard arrow and
    pointing-hand cursors extend downward from their hot spot, so space above the element is never covered by the cursor
    whatever its size, while a box just below a 16 px icon would sit under the cursor's body.
  - It is never clipped by a scroll container: in particular `.app-sidebar` is `overflow: hidden auto`, which clips
    anything anchored inside a row, so the tooltip is a single body-level element positioned with `position: fixed` from
    the target's measured rectangle. The closest precedent is the terminal's link-target display, `showLinkTarget` in
    `crates/farhelm-ui/assets/terminal-links.js` (a measured, viewport-clamped `role="tooltip"` element); the row `⋯`
    menu's popover solves clipping too, but from Rust, so it does not transfer to a script.
  - Peer-supplied text (session titles, paths, agent invocations) stays escaped through `display_peer`, as today's
    native tooltips are. The script sets the text with `textContent` on an element styled `unicode-bidi: isolate`, so a
    tooltip cannot reorder the page around it. Isolating each peer run inside a mixed string
    (`"{cwd} — click to
    copy"`) is not required; today's tooltips cannot do it either, and structured tooltip
    content is out of scope.
- Native tooltips are gone: no element in the UI keeps a `title` attribute used as hover help, so the browser's slow
  tooltip never shows on top of ours. Every existing hover text moves to the new tooltip (agent and permission marks,
  status dot, compact ended-status, stale qualifier, activity time, folder line, cwd on compact rows, restart buttons,
  header copy buttons, text-size buttons, settings gear, host update button, recent setups, and anything else found).
  That includes the titles that reveal truncated text in full (the titlebar's session title, the row and host menu
  headers through `PeerLine`'s `peer_tooltips` prop in `peer.rs`, the row host name), the status badge, the new-session
  form's recent destinations and folder entries, and the app bar's version label.
- Two existing custom hover displays stay as they are and get no `data-tooltip`, so two popups never stack: the hosts
  panel's update-progress popup (`UpdateProgressPopup` / `UpdateProgressLabel` in `crates/farhelm-ui/src/hosts.rs`,
  structured multi-line content) and the terminal's link-target display (anchored to the pointer, carrying its link
  warning). The coverage test tolerates both.
- Coverage: every visible clickable control has hover text, including plain-word buttons such as "cancel". The text says
  more than the label where it can ("stop: end the agent; terminal tabs keep running", "cancel: close without
  renaming"), and is never just the label repeated when there is something more useful to say. This includes the
  sidebar's local and remote host icons and the two buttons the terminal script creates itself ("reconnect now" and
  "take control" in `crates/farhelm-ui/assets/terminal.js`).
- Exemption: items in a `⋯` menu (session row and host row) that already show a description line under their label get
  no tooltip; the description already says it. The `⋯` buttons themselves are covered.
- Accessibility does not regress: anything a removed `title` was the only accessible source of (an icon-only control
  with no `aria-label`, a status dot's "mark read" meaning) keeps an accessible name or description through
  `aria-label`, a `.visually-hidden` sibling, or similar. SPEC.md already requires hover text to use "the same status
  and permission meaning the row exposes to assistive technology"; keep that true.
- A browser test visits the main screens and fails on any visible clickable control without hover text, honoring only
  the menu-item exemption above. A browser test also shows the tooltip after hovering, checks it is above the element
  and inside the viewport, and checks the instant follow-on.
- SPEC.md and SPEC_impl.md describe the tooltip where they now say "a tooltip" or name `title` targets, the TODO.md
  entries named under PR discipline are removed, and a changelog fragment exists.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term, "Hover help on icons and buttons"), verbatim: "Hovering any of these should show a
  short text saying what it is or does: the icons that show a session's agent kind, the YOLO / not-YOLO / unknown
  (question mark) icons, and every clickable button. Today a user has to guess what an unlabeled icon means or what a
  terse button will do before clicking it. Native `title` tooltips are the cheap way to get there; the Maybe later entry
  on custom hover tooltips covers the faster, themed alternative if the browser's built-in delay proves too slow."
- M2. On the native delay: "the reason I didn't think we had any is that it takes FOREVER to show up. we need to make
  that faster." This plan therefore absorbs the Maybe later entry "Custom hover tooltips on buttons and menu items".
- M3. Coverage: every clickable control including plain-word buttons, with text that says more than the label where it
  can; `⋯` menu items that show a description line are skipped; the sidebar's host icons and the terminal script's own
  buttons are included.
- M4. Delay 300 ms; instant follow-on between controls; also on keyboard focus; every existing native hover text moves
  to the new tooltip.
- M5. Placement above, falling back to below with a larger gap; the look described under The goal. The maintainer raised
  the cursor-covering problem with below placement and agreed with the above-first rule.
- M6. A browser test that fails on any visible button without hover text.
- M7. Runs after `launch-representation.md`.
- M8. Review gate: two fresh-context reviewers per PR, a Claude Opus 5.5 agent at high effort and a gpt-6-astra agent at
  high effort, both with the general review charter (below).
- M9. No-workhorse mode (below).

Binding repository rules:

- Desktop asset parity: a new asset file is declared through the crate's asset-enrolling macro in
  `crates/farhelm-ui/src/lib.rs` (read its doc), and `scripts/check-desktop-assets.sh` must keep passing.
- Peer text goes through `display_peer` (see `PeerTitle` and the comments in `crates/farhelm-ui/src/list/row.rs` about
  tooltips and direction isolation).
- Root `AGENTS.md`: changelog fragment for a `feat` PR; TODO entries removed in the PR that addresses them; "Finishing
  work" for validation; `.agents/test-authoring.md` for test changes; `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md` when browser tests change; browser specs on Chromium and WebKit through the recorder.
- The JS unit harness `crates/farhelm-ui/js-tests` tests pure functions of asset scripts (`cd` into it, `node --test`).

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION):

- P1. One new asset script, `crates/farhelm-ui/assets/tooltip.js`, with a single delegated listener set on `document`
  and a single tooltip element (`role="tooltip"`) appended to `body`. Elements opt in with a `data-tooltip` attribute
  (not `title`, which would bring the native tooltip back). Dioxus components set `data-tooltip` like any other
  attribute; the terminal script sets it on the buttons it creates. No Rust-side tooltip component is needed, which
  keeps the mechanism out of every component's props.
- P2. Keep placement a pure function (target rectangle, tooltip size, viewport size → position and side) exported for
  `crates/farhelm-ui/js-tests`, so above/below/sideways-shift is unit-tested without a browser; the browser test then
  only proves the wiring.
- P3. Warm follow-on: remember when a tooltip last hid; a new target within about 300 ms of that, or while one is
  showing, shows at once.
- P4. Disabled controls: some engines dispatch no pointer events on natively `disabled` buttons. Give them
  `data-tooltip` like any other control, so the text is there once they enable, and convert nothing to `aria-disabled`
  beyond what already uses it (`restart-with-trigger` in `crates/farhelm-ui/src/session_view.rs`). Nobody asked for
  disabled controls to explain themselves on hover, and a conversion means guarding clicks and keyboard activation.
- P5. Accessibility: the tooltip is a visual aid. Do not wire `aria-describedby` dynamically unless a removed `title`
  leaves something with no accessible text; then prefer a static `aria-label` or `.visually-hidden` text, matching the
  codebase's existing pattern.
- P6. Texts follow the UI's existing tone (short, lowercase like the labels they sit beside). Keep the meaning of
  existing texts such as `permission_description` in `row.rs`; only their delivery changes.
- P7. The coverage test enumerates visible elements matching a fixed selector list
  (`button, [role=button],
  [role=tab], [role=menuitem], a[href], select, summary`; no `cursor: pointer` heuristic) and
  asserts a non-empty `data-tooltip`, exempting described menu items by their class. Disabled controls count for the
  attribute only; the test does not require the tooltip to show on them. It visits only states the existing e2e helpers
  and fixtures already reach (for example the sidebar with sessions in both row densities, a session view with tabs, the
  hosts panel, settings, the new-session form, the rename and confirmation dialogs). Build no new fixture infrastructure
  just to widen its reach; the inventory pass and the reviewers cover what it cannot reach. Exemptions are an explicit
  list in the test, each with its reason.
- P8. Rust render tests in this crate use `VirtualDom::rebuild_to_vec()` and assert `SetAttribute` mutations, which only
  show dynamic values (see `titles_render_escaped_in_the_row_and_both_confirmations` in `row.rs` and the `PeerTitle`
  doc). Existing Rust and Playwright tests that assert `title` attributes move to `data-tooltip` (Playwright specs found
  at planning time: `terminal.spec.ts`, `omp-composer.spec.ts`, `restart-with.spec.ts`, `sidebar.spec.ts`,
  `header.spec.ts`, `clone.spec.ts`, `terminal-restart.spec.ts`, and `profiles.spec.ts` if it survives the launch
  redesign; search for `"title"` rather than trusting this list).
- P9. Add a tooltip check to `docs/manual-mac-checklist.md`, since the desktop app's WebKit is not otherwise exercised
  for this (the WebKit browser run stands in for it in automation).

Agreed fallback: if a control is so dynamic that a meaningful hover text cannot be given (for example a list item whose
whole content is its label and whose action is obvious), the coverage test may exempt it only with a recorded reason and
a DECISION in the log; the report lists every such exemption for the maintainer.

## Implementation outline

UI only: one asset script, CSS for the tooltip, attribute changes across `crates/farhelm-ui/src` and
`crates/farhelm-ui/assets/terminal.js`, and tests. No helm, supervisor or protocol change, no persistent state. The
mechanism is roughly a hundred lines of script plus its CSS; the coverage pass is wide but shallow (about 110 controls
across `app_bar.rs`, `auth.rs`, `settings.rs`, `rename.rs`, `yolo_confirm.rs`, `provisioning.rs`, `restart_with.rs`,
`launch_controls.rs`, `tabs.rs`, `hosts.rs`, `hosts/settings_dialog.rs`, `list/view.rs`, `list/row.rs`,
`list/create_form.rs`, `session_view.rs`, `status.rs` at planning time, adjusted for what the launch redesign landed).

Suggested stack, bottom up:

1. `feat`: the tooltip mechanism, its CSS, its JS unit tests, migration of every existing `title` hover text to
   `data-tooltip`, the browser test for show/placement/follow-on, SPEC.md and SPEC_impl.md wording, the changelog
   fragment, and removal of the Maybe later entry "Custom hover tooltips on buttons and menu items".
2. The coverage pass and the coverage browser test, removing the Near term TODO entry "Hover help on icons and buttons".
   Split it by area (sidebar and session view; hosts, settings, dialogs, new-session form) only if a reviewer would
   genuinely be helped.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-hover-help-log.md` in the
parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/hover-help/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; the `feat` PR carries its changelog fragment under `releasing/changelog.d/` in the same commit,
  per root `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- Remove the TODO.md entries this plan covers in the PRs named in the outline: Near term "Hover help on icons and
  buttons" and Maybe later "Custom hover tooltips on buttons and menu items".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `scripts/check-desktop-assets.sh`, a nextest selection of `farhelm-ui`,
`cd crates/farhelm-ui/js-tests && node --test`, `python -B scripts/check-test-sleeps.py`, `dprint check` on changed
files, and the new and changed Playwright specs on Chromium and WebKit through the recorder (this change is browser
behavior, so browser evidence is required here, not optional). Say in the report which specs ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (M8): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a Rust tooltip component threaded through props, a
positioning library, a tooltip that holds interactive content, per-element configuration of delay or side, a change
outside `crates/farhelm-ui` and `e2e`; these are examples, not a blacklist), and whenever the same component has needed
repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the
request, the decisions above, this outline, the current diff and the proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the attribute and script shape (P1), the follow-on grace period, how disabled
controls get hover text (P4), every accessibility substitution for a removed `title` (P5), every coverage-test
exemption, the SPEC wording, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The one agreed fallback is the recorded coverage exemption under Decisions already made.
Anything else that needs a decision (for example, the delay or placement rules turning out unworkable in WebKit): record
the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed both TODO.md entries. Open, not merged: merging happens only after the maintainer has reviewed
this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
