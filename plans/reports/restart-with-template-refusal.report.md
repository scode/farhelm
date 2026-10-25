## What this was about

Restart with refused a template whenever it named a choice the session keeps fixed, even when it repeated the current
value. Every template saved from the session launcher records its tab, so those saved templates could not be reused
there. Replace with also refused a template naming the session's own host.

The maintainer chose to accept equal fixed choices as no-ops while keeping real changes atomic refusals. Templates and
their shortcuts now follow that rule in Restart with, and Replace with accepts its own host. Other edits in the same
template still apply. A template containing only equal fixed choices applies silently and leaves Restart inactive.

## Things you should know

Matching an agent never reselects it, so an older session's unset permissions, model and effort survive unless the
template explicitly edits them. Host matching requires a usable recorded identity. Directory matching compares the same
text ignoring trailing slashes, without expanding `~` or resolving symlinks; session names compare with their raw text
exactly. Initial messages and clearing a command session's resume choice remain refused.

Search results and recent setups retain their previous presence-based refusal in Restart with. The helm's
`farhelm spawn` behavior is unchanged. A template or shortcut still leaves the restart action to the user. The product
and implementation specs and the Launch templates docs page now describe the chosen behavior. The TODO entry is removed
and a fixed changelog fragment is included.

## Open questions and possible follow-ups

None identified within this plan's scope.

## The PRs

- [#2711: reuse matching templates in Restart with](https://github.com/scode/farhelm/pull/2711/changes), open draft.

## Checks run, reused and skipped

- Focused UI draft and launcher unit tests through the recorder, using
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::draft::tests::) | test(launch_composer::)'`: run
  `7e880ce3-7cf6-4b2f-b34b-7521c2f5d20a` passed 111 selected tests, with 654 cases selected out, zero failures, errors
  or flaky outcomes, four slots and zero retries. Source capture, report and retained output are complete; required
  pinned nextest and tmux were verified, and no runtime `SKIPPED` marker appeared.
- Browser validation through the recorder, selecting
  `npx playwright test 'restart-with\.spec\.ts|templates\.spec\.ts|launcher-template-shortcuts\.spec\.ts' -g '[Rr]estart.with|Replace with accepts its own template host|Replace with refuses a whole sequence'`:
  run `6989fb84-d0e6-4dc2-8154-416642ed4311` passed all 110 cases, 55 on Chromium and 55 on WebKit, with zero failures,
  skips, flaky outcomes, interruptions or global errors. Three worker-owned stacks, zero retries and required pinned
  tmux were used. Source capture, runner policy, report and retained output are complete, with stream EOF observed and
  no runtime `SKIPPED` marker. This covers the full Restart with spec and the affected Replace with template cases.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. Its first attempt rejected the expanded result
  tuple's type complexity; a documented type alias resolved it without changing behavior. The release Dioxus web build
  passed.
- `cargo fmt --all -- --check`, dprint on the changed specs, TODO and docs page, and
  `python3 releasing/check-changelog.py format` passed. Dprint excludes TypeScript and fragments, so it provides no
  formatting evidence for those; the separate fragment lint covers the changelog. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 315 delays with zero missing rationales.

The runtime records capture main `3b071c8a` plus this plan's working diff. The Rust record's source fingerprint is
`cebfaa103af21272ee2dc62e79ef3166f421bac77e989fc61d660b61a340b7e2`; the browser record's is
`40302fd6cac63375c1e761062e1567f81e267db20bc16349ea418722071d8300`. The only later Rust change is the result type alias;
the Rust tests and built web behavior remain applicable. The browser run uses the corrected fixture that changes effort
while repeating kind and agent. The native backend build is reused from `b2b31b184ee5d5db3102b18ae18f968befe3c264`: this
plan changes no backend executable behavior, and the shared protocol edit is documentation only.

The final PR head is `fd35b64452629c43359f263762e1288d5a6fc8e4`. Its patch is the reviewed patch rebased onto
`e9a28b7f`; the full upstream delta contains only another plan's queue delivery and report, so it has no executable,
specification or test interaction. Each check is retained across that bookkeeping-only rebase without repeating it.

The full Rust and browser batteries, desktop smoke, installer and release checks were skipped because the changed shared
launcher path is covered by focused unit tests and both browser engine families, with no desktop-shell or backend
behavior change. The website build was skipped for this incidental wording-only edit, which changes no links, headings,
frontmatter or generated artifacts. No instruction file, heading or file location changed, so the agent-instruction
checker was not needed. No preview server was started for the incidental docs edit.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort received the complete charter, goal, settled
decisions and verbatim test-authoring checklist. It found one browser fixture that combined the equal-agent permission
check with an explicit model edit, which legitimately changes those permissions. Removing that model edit retained the
required launcher-shaped kind/agent/effort case and isolated the intended guarantee. The same reviewer checked the
correction and type alias and passed the gate with no unresolved findings. There were no rejected findings, scope
departures or weakened guarantees. Commit and PR wording received a separate cold read, and every touched file received
a separate documentation pass.
