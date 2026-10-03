## What this was about

The installer tried to cover several installation setups, and its completion message mixed results, restart instructions
and setup advice into a wall of text. The maintainer chose to limit it temporarily to a Mac installing the desktop app,
remove two installation options, and use the approved short messages with terminal styling and download progress.

## Things you should know

The installer now supports Apple silicon Macs, including a shell running under Rosetta. Intel Macs still have no release
build. Linux and other operating systems are refused before downloads or filesystem changes. This limits only the
installer: Linux remains supported for helms and session hosts, including provisioning hosts over SSH.

Every successful installation puts the command-line tool and desktop binary in `~/.local/bin` and builds
`~/Applications/Farhelm.app`. `FARHELM_INSTALL_DIR` and `FARHELM_NO_APP_BUNDLE` are gone; `FARHELM_VERSION` remains.
Releases without the Mac app icon are refused before replacing anything or acquiring the installation lock. This check
requires downloading and extracting the release first, so a rejected old release may leave the bin directory created;
staging is cleaned up. The supported release floor is 0.2.1.

Fresh installs and updates now show the approved short instructions, a full-path uninstall command, and Homebrew advice
when tmux is missing or too old. Terminal stderr shows labeled progress for the two archive downloads; redirected output
has no terminal controls. Nonempty `NO_COLOR` disables color and emphasis, while a terminal's Homebrew link stays
clickable as explicitly decided in the plan. The styled output was exercised under pseudo-terminals and inspected.

A file the installer cannot vouch for is still kept under another name and reported. It no longer makes a fresh install
look like an update: only an existing binary matching the installation record triggers update wording. Older
installations without a matching record therefore conservatively receive the fresh-install instructions.

Linux installed-uninstall acceptance coverage goes away with the Linux installer, including the end-to-end test of Linux
service-failure ordering during uninstall. Linux uninstall behavior itself is unchanged. Native macOS installed
acceptance passed, and the shell fixtures retain rollback, integrity, ownership, legacy-record and foreign-file checks.
The specifications now state the reduced acceptance coverage.

## Open questions and possible follow-ups

No decision blocks these drafts. Two wording edges are preserved for the maintainer's review. When updating with tmux
missing or too old, the report keeps the approved restart/session-survival sentence and also repeats the restart
instruction after the tmux remedy. Removing the earlier sentence would remove its session-survival assurance. A
malformed or empty `tmux -V` result stays raw, as the plan explicitly requested; multiline output can disrupt the tidy
layout, and empty output produces an awkward version sentence.

Safe updating while Farhelm is running remains a separate queued plan. This work adds no running-app detection,
versioned update staging or restart coordination. The website remains outside this plan's scope.

## The PRs

- [#1524](https://github.com/scode/farhelm/pull/1524/changes): limit the installer to the Mac desktop setup and update
  its documentation and fixtures.
- [#1526](https://github.com/scode/farhelm/pull/1526/changes): use the approved short, formatted installer output. This
  draft is based on #1524.

Both PRs remain drafts; neither was marked ready or merged.

## Checks run, reused and skipped

- PR 1's final installer shell suite passed 541 checks, recorder `9caff083-4084-4c43-92aa-b74b1c94de77`. PR 2's final
  full shell suite passed 567 checks, recorder `71c33dde-397b-4b30-8af8-b9c8c72e833d`, covering the corrected
  fresh/update distinction, exact report text, independent stdout/stderr terminal decisions, progress and error
  prefixes. Narrow review-correction checks also passed, recorder `95694fb5-f9e0-454f-973f-5d6f4ccafb6c`.
- The focused [macOS uninstall workflow](https://github.com/scode/farhelm/actions/runs/37150576753) passed on PR 1 at
  `17f3a4995d56`: 35 uninstall unit tests and eight installed acceptance cases. Recorders
  `e11826bd-381f-4607-b245-6e8ba255f87d` and `18814fd8-14f6-4a46-8be9-e06c9b8e39d3`. Reused for PR 2 because it changes
  reporting and styling, not ownership, replacement or removal. Shell tests cover the new report classification.
- The Rust release-asset parity test passed, recorder `a023ce4d-0952-4136-983b-040ea8404717`. Reused after later changes
  because the release table and its extraction wrapper stayed unchanged, including the Linux rows used by provisioning.
- Shell syntax, ShellCheck, changed Markdown formatting, changelog format and whitespace checks passed. Rust formatting
  and the isolated test-delay check passed for PR 1; the latter found 274 delays with zero missing rationales. PR 2
  changes no Rust or browser tests, so those checks were not repeated.
- Earlier failed observations are retained: PR 1 recorder `0a4541e4-f68c-477a-b431-a3378f2375f8` exposed six stale
  fixture assumptions during the macOS conversion; PR 2 recorder `017cea64-eabd-4ad4-80b4-42fab3b49c40` had one stale
  assertion expecting the old update message. Corrections passed narrow retries and the final suites. These are
  development failures, not latent flakes or successful runs.
- The stack was rebased onto `0bd09e68`. Every incoming diff was inspected: queue approvals, a delivered report and a
  TODO about the local host's update action. None changes installer contracts. The rebase preserved installer code,
  fixtures, specifications and validation inventory byte-for-byte, so runtime results were reused.
- Full Rust, browser, desktop runtime, website and release batteries were skipped: this change affects the installer and
  its reports, with focused asset parity and native uninstall evidence covering their relevant interactions.

## Review gate's outcome

Each PR received a fresh-context Opus 5.5 review at high effort. PR 1's findings improved the documentation of removed
Linux acceptance coverage, old-release preservation checks and fixture premises. PR 2's blocking finding corrected the
fresh-install/update distinction; its other accepted findings strengthened exact-text and terminal tests, error prefixes
and comments. The final tests passed after those corrections.

The raw tmux wording and repeated restart instruction were retained for the reasons above. A suggested defensive branch
for a hypothetical third download was declined: the installer has exactly two supported Mac assets, and the label order
is documented. No new mechanism was needed. Both PR titles passed separate cold reads.
