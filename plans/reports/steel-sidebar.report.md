## What this was about

The sidebar now follows the approved Steel design: the chosen sort order is its title, a status filter sits below it,
and a permanent strip of host icons replaces the host dropdown. Hosts at the strip's foot opens the existing host
management controls in place of the session list. The work uses the Steel foundation's colors, fonts and shared
controls.

Filtering keeps the open session and terminal. Choosing a hidden session from the quick switcher clears the host and
status filters when needed; visible choices preserve them. Status sounds continue to follow both filters. Host actions,
their confirmations and running work keep the same owners when switching between Sessions and Hosts.

## Things you should know

The three PRs below replace the outline's four: the unpublished host strip and management panel were combined after a
scope review because their menu origins and presentation depend on each other. This avoids adding a temporary focus
interface and removing it in the following PR. No planned feature was dropped.

The host strip is 56 pixels wide. Sidebar limits are now 296–656 pixels, default 396, preserving the session column's
previous 240–600 pixels and default 340. Stored widths keep their old meaning. At the minimum, header controls may wrap
as groups and remain reachable. Running setup or update progress gets a line below the host's name so it cannot squeeze
the name away. Host identifiers use the technical font but may wrap at narrow widths to stay contained.

Hosts opens on demand; Sessions is the initial view. The toggle keeps keyboard focus on either transition. Host icons,
All, and accepted session navigation return to Sessions. A refused action preserves the current view. The remembered
host filter gets a quieter outline while management is shown; its filter choice does not change. A context menu does not
choose that host. Async refusals remember whether the accepted action came from the host icon or management row and
return focus to a currently visible control.

Renaming a host or changing its phase preserves focus on its stationary icon. Reordering the host list can make a moved
icon lose focus; arbitrary focus retention across that structural change was not added as a new facility.

The selected session can be stale while its host is actively reconnecting. The icon reports that connecting phase; the
distinct unreachable phase is dimmed and uses a warning dot. Captures do not reclassify a failed attempt as an
unreachable host just to demonstrate that look.

## Open questions and possible follow-ups

No maintainer decision is needed for this delivery. The remaining Steel plans own the session toolbar, dialogs and
sentence case. In particular, inherited host/help menu wording was left for the sentence-case plan; the 650-pixel
toolbar still has the pre-existing cramped text-size action that the session-chrome plan addresses.

The visual reviewer noted two nonblocking spacing nits: the running host's name starts about 3–4 pixels to the right of
the other names, and Details has a wide gap beside it at the maximum sidebar width. Both remain readable and reachable.
These were left within the mockup's approximate geometry instead of starting another cosmetic capture round.

Normal web appshots cover the layout forced into the macOS title-band shape. Native traffic lights and AppKit drag/zoom
were checked from code, not run on a Mac. The current staged hosts do not show an update-available badge or a remote
with a custom color; source and focused state tests cover those roles. No install, SSH configuration or live Farhelm
state was changed. The setup pictures use the existing simulated host-action backend with real helm progress.

## The PRs

- [#2747](https://github.com/scode/farhelm/pull/2747/changes): sort title, count and compact icon actions.
- [#2749](https://github.com/scode/farhelm/pull/2749/changes): status filtering and quick-switcher behavior.
- [#2750](https://github.com/scode/farhelm/pull/2750/changes): host strip, management panel, widths and title-band
  layout.

All remain drafts. The landing monitor owns their merge.

## Checks run, reused and skipped

All runtime commands used the recorder. Browser commands used Chromium and WebKit, three workers with one owned stack
each, pinned tmux, zero retries. No full browser or workspace battery was run as a finishing ritual.

The header run group covers PR #2747 as it developed through pre-rebase head `c4765fbf6d3a2391b34e1360606d3de0579b11c0`;
its final reviewed head is `08b93af1c7370bcdb1e47e03394b591cb6f6b161`. The status group covers PR #2749 through
pre-rebase head `f90b7a72ff31391ca57c6e8f3aa77e48848087e7`; its final head is
`451963c369e8c8be92f6ca82830d8e8f2c3f5193`. The host group covers the final pre-rebase working-copy revision
`7b1abd06f0f990f3b283f6bc289eacf76817d8cb`; PR #2750's final head is `c71a79493377f6061d7763b5fc9d611b80fe499f`. All
three were rebased onto main `a46397c85637096e56898aecb914d1f8f3ce7d74`.

Individual commands ran while those revisions were being developed, so their retained source fingerprints identify the
actual dirty working copies rather than treating Git HEAD alone as the tested tree. For example, the final 18-case
browser command, final Clippy command and final capture all bind source fingerprint
`56dfd5bde1a92a3c1e1a1e773a47510686e58bd8a9326efb7548ab249fd540f8`. Earlier commands are reused only for the unchanged
behavior named below, with later affected behavior covered by the narrower runs.

- Header: `cargo nextest run -p farhelm-ui --lib` passed 784 tests, run `d5d3c10b-3a88-4730-9745-ace4c3e3c337`; the UI's
  `node --test` passed 358, run `c030afe6-17d9-4391-acc8-161be1b00cb9`. Sort/filter browser specs passed 56, run
  `f723fdd2-3328-47d9-86f5-498176931ae5`; selected compact/count/wrapping cases passed 16, run
  `1c8c569a-0bf0-4996-8c6d-cae458082cc2`. Reused for the unchanged header behavior above those revisions.
- Status: selected status/quick-switcher/sound browser cases passed 16, run `72039ee1-912c-43c6-9f65-0beed4156359`;
  focused current-list membership units passed, including final run `12edf6e3-8575-4c4e-9555-be9292fa31ae`. Reused
  because subsequent host presentation changes preserve their query and admission paths.
- Hosts: width/token JavaScript tests passed 19, run `4f826f1e-d4af-46ba-a975-49eecc1204b9`; phase/progress units passed
  2, run `58649fe2-f648-4445-b96e-6b7eb3c69e95`. Selected browser cases passed 80, run
  `ee4e5565-f943-4b44-8df9-582a28bc43b2`. After visual corrections, the affected menu, panel, running-progress,
  keyboard-focus and movement checks passed 18, run `0fd5f8cd-d39f-4020-86c8-a2cc9a9b14c1`. The earlier 80-case command
  is reused where those local styling corrections leave its behavior intact.
- Final `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed, run `32050de7-53a4-4e83-adab-834f418c1b23`;
  the matching release web build passed, run `4329d3af-cf02-491a-a967-b2a7d9728702`. Rust formatting, changed Markdown
  formatting, changelog format and the test delay checker passed; the delay checker inspected 322 annotated delays with
  none missing a rationale.
- Ordinary appshots: the header's 32 views and status filter's 48 views passed their capture commands. The final host
  capture produced 84 views in dark/light at wide and 650-pixel main widths, including minimum/maximum sidebar widths,
  compact rows, menus, settings, setup progress, retained stale state and the Hosts panel. Final capture run
  `f5ee040a-0449-4a7d-93e6-ef902556d2e7` passed all five capture cases.
- Earlier fixture failures and stale-build mistakes were retained and corrected before narrow reruns. An additional
  14-case command, `6cfd18c2-2210-4461-8025-0c7bcf14cb78`, was externally interrupted after 13 passes; it is not a pass.
  Its interrupted progress case passed in the final 18-case command. Failed or interrupted whole commands were never
  reused as passing evidence.
- Rebased all three PRs onto current main after reading its complete intervening diff. The change for each PR is
  identical before and after rebase; no conflict resolution was needed. The installer, Restart trust/reporting and
  hidden-trash backend changes are independent of sidebar presentation. The link-refusal layer and template deletion
  changes use different UI owners and stylesheet sections. The trash fixture's path rename does not change the
  header-count geometry it measures. The commands above are therefore reused across the rebase; no extra runtime battery
  or appshot capture was needed for it.
- Skipped: full Rust/browser batteries, native desktop runtime, installer/provisioning distribution tests and real
  vendor agents. The change is local UI presentation/query selection; focused tests cover the changed contracts, and the
  host action backend, install and vendor launch behavior are unchanged. No helm filtering change was needed.

## Review gate's outcome

Each PR received the required fresh gpt-6.1-sol/high code review and claude-opus-5-5/high multimodal visual review. Code
findings were fixed and rechecked; no code finding remains. Opus inspected the header's 32 views, the status filter's 48
views and the host interface's 84 views against the rendered references. After fixes, it inspected all 36 changed host
views again; the other 48 reused the initial inspection. Actual image delivery was verified. It cleared the final views
with the two cosmetic nits recorded above. The phase distinction, minimum-width wrapping, narrow identity wrapping and
deferred inherited menu case were explicit decisions, rather than silent omissions. Commit/PR wording passed its
independent cold reads.
