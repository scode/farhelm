# Hosts needing attention above sessions

## What this was about

Setup progress and host problems were easy to miss while Hosts was closed. Updates had separate banner and card
reminders. The requested change puts hosts needing attention together above the sessions, while keeping the full
host-management panel available from Hosts.

## Things you should know

The list shows problems, ongoing setup/update/uninstall work, and available updates. A host appears once: ongoing work
takes precedence over a problem, and a problem over an update. Problems appear first, followed by work and updates.
Choosing a host opens Hosts at its menu button without changing the session filter. Working rows show the operation and
progress; their full step details remain in Hosts.

Update acts on one shown host; the list's Update all acts on exactly its shown Update rows and appears only when there
is more than one. The broader Update all inside Hosts keeps its existing scope. Optional update reminders can still be
dismissed for the current interface lifetime; required updates, problems and work cannot. Reloading, a new update
identity or a changed helm build can reveal optional reminders again.

The mockup's unreachable-since timestamp was omitted as the plan directs: Farhelm records no such time. The existing
retry wording is retained. Ordinary Connecting and suspended hosts, healthy hosts and usable newer hosts do not appear
unless they have an independent warning or ongoing work. The banner and card are removed. No schema, protocol,
persistent preference or host-operation policy changed.

The implementation keeps the existing mounted operation owners and warning acknowledgement. A review found that
scrolling a whole expanded host row could leave its focused menu offscreen; the handoff now scrolls the menu button
itself. A browser regression proves the tall, straddling-row premise and visible focus in both engines.

## Open questions and possible follow-ups

No blocking question remains. The existing images on the two changed host guides show Hosts, dialogs or the navigation
rail; none was confirmed stale from this change. A dedicated illustration of the attention list is still absent. No
documentation capture or preview was started during this unattended run. The Mac app itself was not driven; WebKit
browser coverage and desktop compilation are the available evidence.

## The PRs

- [#3044: show hosts needing attention above sessions](https://github.com/scode/farhelm/pull/3044/changes), open and
  draft.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --lib -E 'test(/^hosts::/)'`: 92 passed, 769 selected out; recorder
  `61bb17c3-e346-4f2f-9e5a-3a15d1daf22d`, four slots, zero retries, no tmux required. This covers classification,
  warnings, host projections and update authority. Reused after the focus-only correction and added browser regression
  because those Rust contracts did not change.
- Chromium and WebKit, expanded-host navigation regression: two passed; recorder `21b718b4-ac1c-4989-b582-ceaf174d7302`,
  pinned tmux 3.7c, zero retries. Both engines exercised the oversized-row and pre-scroll geometry assertions.
- Chromium and WebKit, `host-updates.spec.ts`, `host-rail-warning.spec.ts`, `hosts-panel.spec.ts` and
  `provisioning.spec.ts`, excluding the already-passed expanded-row case: 224 passed, 112 per engine, with no skips or
  failures; recorder `53dfe9cf-fcbb-45f8-ac80-621012451f2f`, three owned stacks, zero retries, pinned tmux 3.7c. Four
  retained short-sidebar screenshots were inspected: both palettes in both engines keep the header actions and sessions
  usable while long host names truncate and the host rows scroll.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed on the final PR revision, including the corrected
  focus handoff.
- `cargo check -p farhelm-ui --features desktop`, final web build, backend and fixture prerequisite builds,
  `cargo fmt --all -- --check`, supported changed-file Markdown formatting, TypeScript formatting, changelog-fragment
  format and the isolated test-sleep checker passed. The checker inspected 412 delays with zero missing rationales. CSS
  and MDX have no configured formatter here and were reviewed directly.
- The ordinary workspace and full browser batteries were skipped: the affected host flows and direct
  classifier/navigation checks cover the changed behavior. Backend runtime suites were unnecessary because no backend
  code changed. No website build was needed because the guide edits change no links, headings or frontmatter. CI is
  manual-only and was not dispatched. No live installation was changed.

Runtime evidence was collected on the plan's base plus its working changes. Subsequent main updates changed another
plan's tracking, unrelated quick-launch cleanup, and a startup check used only by the separate host-terminal browser
run. This plan's ordinary browser run does not enter that check. The careful rebase preserved both sides of shared
follow-up lists and has an empty interdiff for this PR, so its feature and relevant validation remain unchanged.
Complete recorder reports and output were checked; no runtime pass is inferred from a skipped substrate.

## Review gate outcome

A fresh gpt-6.1-sol/high reviewer found one navigation issue, which was corrected with the expanded-row regression. A
second fresh reviewer of that exact model and effort reviewed the corrected whole diff and reported no findings. The
commit/PR text passed its independent cold read. The documentation pass covered every changed code file and test.
