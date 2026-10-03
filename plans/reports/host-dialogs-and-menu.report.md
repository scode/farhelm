### What this was about

Adding or removing a remote host used to put its question inline in the host list. A narrow row could contain the SSH
fields, probing result, setup plan, or removal explanation at the same time as the list, which made the flow easy to
lose and left removal confirmation competing with row actions. In the plan decisions recorded on 2026-10-02, you decided
that adding a host should keep its destination, probe, and setup choice together in a modal; that the setup question
should name only durable changes Farhelm makes on the host; that "yes, and don't ask in the future" is a permanent
helm-wide choice; and that removal should use the same modal pattern with a separate permanent opt-out.

The stack does that. The host menu now follows the session menu's placement, grouping, descriptions, icons, keyboard
behavior, and hover-revealed toggle. Adding a host happens in a modal that carries the SSH fields, probe outcomes, and
setup confirmation. Its confirmation lists the destination's distribution and architecture plus the directories,
Farhelm/tmux paths, user service, persistent startup, and boot-or-login fallback that provisioning changes on the host.
A permanent setup answer is stored in the shared helm preference and skips only that later question; discovery of an
already-running supervisor still adds the host without a setup question, and remote updates still submit without this
add-host question. Removing a host uses a modal with a safe initial Cancel focus, explains that the supervisor and
sessions keep running, and offers the matching permanent removal choice. The helm preference schema, API, specs, website
guidance, end-to-end helpers, and changelog fragments are updated. The TODO entries removed are "Make the host pop-up
menu match the session pop-up menu", "A proper dialog for removing a host", and "A proper dialog for adding a remote
host".

### Things you should know

- The permanent choices are shared by all clients of one helm and are best-effort writes. If persistence fails, the
  current add or remove still proceeds and the question can appear again later.
- There is intentionally no UI to turn either choice back on yet; the remaining follow-up is recorded in the near-term
  TODO work that is outside this plan.
- Browser end-to-end tests were not run because the checkout does not have the Playwright browser dependencies. The
  changed browser helpers and specs were checked statically and the Rust/UI behavior was covered by focused tests.

### Open questions and possible follow-ups

None require a maintainer decision for this stack. When the Playwright browser dependencies are available, run the
affected Chromium and WebKit host-menu and host-dialog specs before landing; a later plan can also add controls for
restoring setup or removal confirmation after a permanent choice.

### The PRs

- #1472 `feat: match host menu to session actions`: https://github.com/scode/farhelm/pull/1472/changes
- #1496 `feat: add host removal confirmation choices`: https://github.com/scode/farhelm/pull/1496/changes
- #1502 `feat: add host setup confirmation dialog`: https://github.com/scode/farhelm/pull/1502/changes

### Checks run, reused and skipped

- Run on the final reviewed tree: `cargo fmt --all -- --check`; the helm and UI library/all-targets Clippy checks with
  `-D warnings`; focused helm tests for provisioning confirmation text, host distribution/architecture text, the schema
  33 migration, preference routes, and preference-store merging (recorder run ids `28056397-4002-4772-960f-d137ad4cbc2f`
  and `4bcb6178-4e65-40ad-9d56-8aed5f45c02d`); the focused UI preference and remote-update tests (run id
  `e81a0845-7c71-499d-8e02-60965036b443`); `python3 scripts/check-test-sleeps.py` (274 delays inspected, zero
  unannotated); `dprint check`; changelog format validation; and the website build with internal-link validation.
- Run after the final PR 3 review fixes: `cargo check -p farhelm-ui` and `cargo test -p farhelm-ui --lib` (425 passed).
- Reused: the focused helm/UI runs above and the review-gate checks after the careful rebase. Main added an unrelated
  supervisor screen-state wording/formatting change in `SPEC_impl.md`; inspection found no interaction with this stack's
  host-menu or host-dialog contracts, so no runtime rerun was needed after rebasing.
- Skipped: the full workspace Rust battery, doctests, desktop checks, installer checks, and browser end-to-end runs. The
  affected helm/UI paths have targeted coverage, the documentation and website checks passed, and Playwright
  dependencies are unavailable for the browser runs.

### The review gate's outcome

The host-menu review found and corrected unsafe peer-summary rendering, missing menu-group separators, and wording that
called a failed update a setup retry. The removal-dialog review found and corrected the stale preference queue test
name; it also verified safe initial Cancel focus, the schema reopen migration, preference isolation, and removal-dialog
scope. The final add-dialog adversarial review found two defects: setup offers rendered a duplicate Cancel button, and
modal Escape isolation used settings-dialog selectors for the add dialog. Both are fixed, and the final focused UI suite
passes all 425 tests. No unresolved review findings remain.
