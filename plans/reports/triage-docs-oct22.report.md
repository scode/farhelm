# Report: five spec clarifications and two discards from the 2026-10-22 triage

## What this was about

Seven review findings you decided on 2026-10-22, all specification text or review-queue bookkeeping, with no product
code change:

- A user's own tmux server or ssh ControlMaster that a session's agent starts first (agents run without tmux's `TMUX`
  variable, so a plain `tmux` there reaches or starts the user's own server) is ended by Stop, Restart or Delete, along
  with windows the user later opened in it from outside Farhelm. You said this is intended ("If you're running a [tmux]
  from within that Farhelm-managed [tmux], you're on your own"), and asked for it to be written down.
- The spec justified a terminal tab owning the services its startup files start first by saying the agent's launch
  usually starts them already. For tmux that is false: the launch runs the startup files while `TMUX` still names
  Farhelm's private server, so a `TMUX`-guarded tmux autostart fires in the first tab instead, and with `exec tmux` a
  detach ends that tab and the server. You kept the rule and asked for the premise to be corrected.
- A Goose sub-agent or nested Goose run, and a second Pi started under the agent that picks up Farhelm's Pi integration,
  can replace which conversation Resume reopens. You chose to state both as accepted limits.
- A checkout root (the folder managed checkouts are created in) that more than one host can reach (a network mount, say)
  lets one host trash and later delete a checkout another host's session is working in. You chose to declare it
  unsupported, with no detection.
- Two findings you agreed to drop: an open session launcher that is handed a different source keeping the conversation
  picked for the old one (no shipped way to hand it a new source while it is open was found), and Cancel on the
  launcher's loading screen discarding an accepted launch (the loading screen and its Cancel are gone before a launch
  can be accepted).

What landed: each of the five clarifications is written into SPEC.md where the triage entry asked (the two tmux ones
also in SPEC_impl.md and in the TODO.md "Doc todo" item about tab-started services, which was widened and stays open for
whoever writes the user docs), and each of the seven feedback files is removed from the review queue with its triage
outcome marked complete.

## Things you should know

- Your principle for the tmux and ssh ControlMaster case said ending the session "(Stop, Restart, Delete, tab close)"
  ends such a program. The spec already says Stop and Restart leave terminal tabs running, so the new text splits the
  two cases: a program the agent started first ends with Stop, Restart, Replace or Delete; one a tab started first ends
  when that tab closes or the session is deleted. The review caught the conflict in my first draft. If you meant Stop to
  end tab-started programs too, that would be a behaviour change and a separate decision.
- That same text carries the existing caveat for hosts without a usable systemd user manager (macOS, or Linux without
  the user manager), where a detached process that hides its environment can survive cleanup, and says plainly that this
  is intended, not a defect, so the finding does not come back.
- The tab correction keeps the general premise for other services (an ssh-agent, an editor daemon), which the triage
  entry did not dispute, and adds tmux as the routine exception, conditioned on no tmux server of the user's running
  yet.
- The Goose and Pi limits are a new paragraph under "Durability and resume", labelled as an accepted exception to the
  promise that Restart resumes exactly the session's own conversation, so the two do not read as a contradiction. The Pi
  sentence says "replace or withdraw", since an early report from a child Pi can also withdraw the Resume target.
- The shared checkout root is a new paragraph right after the rule that Farhelm owns its managed checkouts, and is added
  to the list of unsupported configurations Farhelm need not refuse.

## Open questions and possible follow-ups

- SPEC.md elsewhere still says integration is never switched on through the environment, while SPEC_impl.md says Goose
  is the one harness that still is (which is the cause of the accepted Goose limit). The reviewer flagged the mismatch;
  reconciling it is outside these outcomes.
- TODO.md's "Maybe later" entry about a nested Grok says Pi is unaffected because a Pi started inside a session reports
  nothing. Next to the accepted Pi limit that is now slightly incomplete. I left it, since this plan does not address
  that entry.
- The "Managed checkouts" section, where the checkout root is configured, has no pointer to the new shared-root rule. A
  short cross-reference would help whoever writes user docs for it; not required by the entry.

## PRs

- [#2626](https://github.com/scode/farhelm/pull/2626/changes)
  `docs: state that a session owns a tmux server its agent starts`
- [#2627](https://github.com/scode/farhelm/pull/2627/changes)
  `docs: note that a TMUX-guarded tmux autostart runs in the first tab`
- [#2628](https://github.com/scode/farhelm/pull/2628/changes)
  `docs: accept that a nested Goose run can replace the Resume target`
- [#2629](https://github.com/scode/farhelm/pull/2629/changes)
  `docs: accept that a child Pi can replace the Resume target`
- [#2631](https://github.com/scode/farhelm/pull/2631/changes)
  `docs: declare a checkout root shared between hosts unsupported`
- [#2632](https://github.com/scode/farhelm/pull/2632/changes)
  `chore: drop the finding about a launcher keeping a stale conversation`
- [#2633](https://github.com/scode/farhelm/pull/2633/changes)
  `chore: close the finding about Cancel on the launcher's loading screen`

## Checks

Run now: `dprint check` on every changed file (SPEC.md, SPEC_impl.md, TODO.md, TRIAGE_OUTCOMES.md and the review queue
index), and `python3 scripts/check-agents-md.py` for the removed queue files and the new spec anchor links, on the final
version of the stack. Both clean.

Reused: none.

Skipped: all Rust, browser, desktop and other runtime tests. The changes are specification text and queue bookkeeping
only, which the repository's rules exclude from runtime testing.

## Review gate

Each PR got a fresh-context Opus 5.5 reviewer at high effort, reviewing correctness, design and idiom; the two
bookkeeping-only discards shared one reviewer, who reported on each separately. None found anything blocking. I applied
every finding that fit the agreed scope: the Stop and Restart correction above (the only medium-severity one), the
systemd caveat and Replace in the tmux text, clearer wording of the tab correction in all three places, the separate
paragraphs and the usual "Confirmed" label for the Goose, Pi and shared-folder text, and "replace or withdraw" for Pi.
Not applied, as outside these outcomes: the environment wording mismatch and the TODO.md Pi note above, a
cross-reference from "Managed checkouts", and a nearby sentence about sub-agent reports that could be read as covering
Goose. The two discards had no findings.

### Landing

Landed as #2626, #2627, #2628, #2629, #2631, #2632 and #2633, merged bottom-up, with the content the report describes
and one review-queue index conflict resolved. It was the second of three plans in this landing round, after
tunnel-port-and-link-ranges and before resume-target-fixes-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained sign-in-preservation-sweep (a browser that signs in again reloads the page, and a
failed GUI preference save is reported and discarded) and two TODO.md commits. A fresh-context reviewer and I read this
plan's specification text against them and against the other two plans in the round:

- None of the sweep's specification changes is in a section this plan edits, and TODO.md keeps both main's new near-term
  entries and this plan's widened "Doc todo" item.
- resume-target-fixes-oct22, the next plan in this round, adds text to the same "Durability and resume" section of
  SPEC.md. The two sit side by side without contradicting each other: this plan accepts that a Goose sub-agent, a nested
  Goose run or a child Pi can replace the session's Resume target, and that plan changes what Restart with forgets when
  Farhelm's hooks are off and which Grok or OMP process may report a conversation.
- The corrected premise about tmux (an agent's launch runs the startup files while `TMUX` still names Farhelm's own tmux
  server, so a tmux autostart guarded by `TMUX` fires in the first tab instead) matches how the supervisor starts an
  agent and a tab today.

#### Fixes made during the landing

None to the plan's text. Its first PR conflicted with main in the review queue's index, because main had removed an
entry next to one this plan removes; the landing kept main's entries and this plan's removals.

#### Possible follow-ups

- Found at landing, nothing to do: the review finding about checkouts on filesystems without creation times still
  mentions, in its triage notes, the finding about checkout roots shared between hosts, which this plan removed. That
  finding is already triaged and planned (checkout-ownership-oct22), whose landing removes the file and the stale
  mention with it.

#### Checks

The round's three plans change the helm's link classification and localhost tunnel port, the supervisor's Restart with
and its checks of which Grok or OMP process reports a conversation, and specification text. This round ran the helm's
and the supervisor's own tests, the browser test for clicking links in a terminal, and the source checks, on all three
plans stacked together on this round's main (which already held sign-in-preservation-sweep). Everything below except the
last item ran before the round's documentation fixes (in tunnel-port-and-link-ranges and resume-target-fixes-oct22),
which change only three docs pages and one changelog fragment:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`
  (tunnel-port-and-link-ranges removes a sentence from the desktop-only tunnel warning), the isolated test-sleep check,
  `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build and the UI's release web build: all passed.
- `cargo nextest run -p farhelm-helm` through the test recorder with pinned nextest and tmux, four slots, no retries,
  run `57e5a94a-29d0-4261-b9a5-697446134c41`: 1291 passed. This covers the link classification and tunnel tests
  tunnel-port-and-link-ranges added or changed, and the rest of the helm.
- `cargo nextest run -p farhelm-supervisor --lib`, same recorder settings, run `8604dc02-2297-49e6-a824-76be3cebf57d`:
  1258 passed, 4 skipped by design (helper entries that only run as child processes of other tests). This includes the
  Restart with tests that drive a real restart and every Grok and OMP process check.
- `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/` on Chromium and WebKit, three workers each with its
  own helm and supervisors, no retries, run `6f350fd5-b9c0-42d3-a88a-f098d376cce2`: 30 passed.
- After the documentation fixes: the docs website build, `dprint check` on the changed Markdown files, the changelog
  format check and the instructions check passed again.
- Reused: none. Skipped: the rest of the Rust workspace (the farhelm binary's end-to-end tests and the UI tests; no code
  outside the helm and supervisor changed apart from removing one sentence from a dialog), the rest of the browser suite
  (only link classification reaches the browser), the doctests (no documentation example changed), the desktop smoke
  test, the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were the formatter and the instructions check; those ran again here on the plan as it landed. The
runtime tests above cover the other two plans in the round, not this one, which changes no code. Nothing in the
executor's report became untrue at landing.
