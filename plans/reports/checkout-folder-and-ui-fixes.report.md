# Checkout folder setup and four UI fixes

## What this was about

A preview of the release notes showed four problems in the app: managed-checkout setup displayed a large command-line
instruction with no way to set the folder in the app; approval cards split ordinary session names; save-as-template
checkboxes appeared above their labels; and Settings still used checkboxes with misaligned sound help. The maintainer
chose switches throughout Settings and made checkout-folder setup a feature: one folder setting for all hosts, editable
in Settings or directly in the launcher, with missing folders created at the first checkout.

## Things you should know

Settings can save or clear the all-hosts checkout folder and shows validation refusals beside the field. The launcher
offers the same explicitly saved field when its selected host has no effective folder. Saving refreshes repository
suggestions and preview without reopening the launcher. Selecting a repository first, or switching hosts with that
repository selected, does not hide setup. An already-submitted save finishes even if Settings closes.

A host with a command-line folder override keeps that folder. Per-host settings and post-clone commands remain
command-line only; the browser receives neither the command nor override values. There is still no default folder.

A missing checkout folder and its parents are created on first checkout with the ordinary umask. Configuration,
discovery and preview create nothing. Existing symlink ancestors resolve canonically. A path containing `..` after a
missing directory (for example, `missing/../checkouts`) refuses, as does a dangling link; existing resolvable paths
containing `..` remain supported. A root changed after preview refuses instead of directing the checkout elsewhere. A
later refusal can leave an empty root. Remote hosts need this release for first-use creation; older supervisors retain
their refusal.

Approval cards keep an ordinary session name whole when it fits and wrap within narrow cards. Template-save choices
place each checkbox beside its caption. Settings switches retain native input behavior, including Space activation,
persistence and refusal restoration; the shared Feedback checkbox keeps its original layout. Checkout notes use ordinary
helper styling. The specs describe these behaviors, and the covered TODO entry is removed in the final code PR.

## Open questions and possible follow-ups

None for this plan. The separate TODO for putting the post-clone command in Settings remains open.

## The PRs

- [#1824](https://github.com/scode/farhelm/pull/1824/changes): approval-card session names use the available width.
- [#1825](https://github.com/scode/farhelm/pull/1825/changes): template-save checkboxes sit beside their captions.
- [#1826](https://github.com/scode/farhelm/pull/1826/changes): Settings uses switches with aligned help.
- [#1827](https://github.com/scode/farhelm/pull/1827/changes): launcher checkout notes use helper styling.
- [#1828](https://github.com/scode/farhelm/pull/1828/changes): Settings explicitly saves or clears the checkout folder.
- [#1830](https://github.com/scode/farhelm/pull/1830/changes): the launcher offers inline folder setup and refreshes
  discovery and preview.
- [#1831](https://github.com/scode/farhelm/pull/1831/changes): first checkout creates a missing folder and parents.

## Checks run, reused and skipped

All runtime checks below ran through the test recorder. Rust used pinned nextest and tmux, four slots and zero retries.
Browser selections used Chromium and WebKit, one worker and zero retries. Changed states were inspected in desktop and
phone screenshots.

- Approval layout: final compactness/name selection passed 4/4 in run `71bbc3c9-f2ad-41c6-b24a-481d6780e4c6`. Six
  unaffected selection/remount/modal passes were reused from `1a458873-469d-4015-a09f-209fc0a5a893`; the final
  requester-only CSS leaves those paths unchanged. The initial regression and the superseded single-column layout
  failures were retained.
- Template choices: agent and command save cases passed 4/4 in `6e286967-91fd-480b-86a4-391532aef51d`, including
  desktop/phone geometry, saved content and no launch.
- Settings switches: final modal/native Space/focus and Feedback layout cases passed 4/4 in
  `84d4518c-9749-4b7e-9abd-065a2217c5fe`. Four removal/sound-persistence passes were reused from
  `1d7c38fb-88ec-476d-aff3-9fa6242b96a7` because the final selector restriction preserves Settings behavior.
  `cargo check -p farhelm-ui --features desktop` and scoped UI Clippy passed. Desktop updater markup compiled; its
  unchanged update behavior did not receive another runtime run.
- Checkout helper notes and preview refusal: 4/4 passed in `642b34a0-6815-4386-84fd-571830662f48`.
- Settings folder: browser save/refusal/clear/reopen, pending-save dismissal, modal and helper cases passed 10/10 in
  `95155ab9-dc59-4a7b-b6c2-2c8b68bc7da2`. Helm route/validation tests passed 6/6 in
  `b2c6f495-627a-431c-998f-6ed86a255a01`, revision observation 1/1 in `1c56c819-3a0a-4665-94fe-e5698b58fb53`, and
  updated advice contracts 2/2 in `9859667c-fa85-4995-a9c0-c5b29cc0dc6a`. Relevant all-target and shipped-bin Clippy
  passed.
- Inline setup: final real discovery/save/preview/override, destination and delayed-preview/host-authority selection
  passed 12/12 in `ac8b1f8a-fb6c-4fca-9c7f-f80aa70522ed`. Helm discovery and pure UI authority tests passed 7/7 in
  `4bf5712b-b311-46b4-9616-3c14ffea3dc2`; reused across the mounted UI-only observation repair because their tested code
  did not change. Final UI Clippy and web build cover that repair.
- First-use creation: final root/title/binding and injected scan-error selection passed 6/6 in
  `c850c880-aa84-45e6-b9b0-0ee5ef166219`, including symlink ancestors, dangling-link spellings, unresolved parents,
  changed binding and archive-name reservation. Real first-use clone/hook/agent passed 1/1 in
  `ccd0dfb0-00ae-4220-8942-38854c3bde08`; reused after the narrow dangling-link and scanner-error refusal repairs
  because plain missing-path resolution, occupancy and create/clone/hook/agent behavior remained unchanged. Final
  supervisor and shipped-bin Clippy passed.
- `cargo fmt --all -- --check`, changed Markdown `dprint check`, `python -B scripts/check-test-sleeps.py` and
  `python3 releasing/check-changelog.py format` passed for each applicable unit. The final delay check inspected 282
  delays with zero unexplained. dprint does not select CSS or TypeScript, so no formatter pass is claimed for those
  files.

Expected before-fix failures and same-session repair failures remain private evidence; none was classified as a latent
flake. Runtime output was checked for early-return substrate skips. The seven-unit stack was carefully rebased onto main
`3f0e21b39e841dde0e1f0ca325ccb8bad70b674f`. Every unit's patch was unchanged. Upstream text safety, menu/clipboard
behavior, helm task ownership and actor cancellation, state/SSH advice, specification and dependency changes were
inspected against the stack; no functional conflict was found. Combined evidence covers the shared field between
Settings and launcher, and configuration/discovery/preview/create across the final three PRs. Runtime results were
reused; final formatting and changelog checks passed after rebase.

The tested, gated unit revisions before that unchanged-patch rebase were, in PR order: `d3b44a02`, `fc246acd`,
`b4673cda`, `8cdb2570`, `3aac7455`, `6982d319` and `8aa7fc79`. Runs reused across an earlier repair captured
working-tree edits on top of their Git HEAD, so HEAD alone is not their tested revision. Their retained source
identities are: approval `1a458873` on HEAD `571c5af5`, source fingerprint
`9670604ad3bff697c2f15de31f3d582934eb297c97b6a53014713e21ccff44c3`; Settings `1d7c38fb` on `fc246acd`, fingerprint
`4dcb4190e47eaf4685290d7a3d43d079106e80efb728fecffc43793ab0fae103`; inline setup `4bf5712b` on `3aac7455`, fingerprint
`cc37629a38386e6febcdb337980169e167578fb5f47b8a9346777fb6ccb897ec`; first-use creation `ccd0dfb0` on `6982d319`,
fingerprint `b3fcb1bb92a21c065cd6aab3dad2a9d6a9992288ebafb3c7528022261b80e0dc`. The bullets above identify which
scenarios were reused and why their coverage survives the respective repair.

Full Rust/browser batteries, installer/provisioning/release gates and desktop runtime were skipped because the selected
checks cover the changed controls, configuration/discovery/preview contracts and filesystem admission behavior. No
installer, protocol, desktop backend, update engine or release machinery changed. Symlink handling was exercised on
Linux; a native macOS runtime run was not performed.

## Review gate outcome

Every PR received a fresh-context source review requested on the prescribed gpt-6.1-sol high route, and commit titles
received fresh cold reads. Source review caught compact-card height, shared Feedback styling, submitted-save
cancellation, publication ordering, and host-setup eligibility issues; the implementation was corrected in its own PR
before proceeding. Dangling-link spellings and scan failure classification were corrected in the final unit. Its final
source and scope gate reported no remaining findings or unnecessary complexity. Documentation passes inspected every
touched file. The orchestrator read the findings and checked them against source and targeted runtime evidence; reviewer
claims alone were not treated as test passes. Native model attribution and usage counters were not exposed, so the
requested routes do not establish exact runtime attribution. All seven PRs are verified open drafts with the intended
heads and linear base chain; the executor marked none ready and merged none.

### Landing

Landed on 2026-10-10 (UTC) as #1824, #1825, #1826, #1827, #1828, #1830 and #1831. The rebase met no conflicts.

#### Fixes made while landing

All three are in #1828, before it merged:

- A supervisor test of GitHub-checkout requests still expected the refusal to say "checkout root"; this stack renamed it
  to say "checkout folder". The plan's own checks ran only the tests it changed, so the combined run was the first to
  see it. The test now expects the new wording.
- A refused folder save with no explanation in the reply showed an empty error under the field. It now falls back to the
  request and its status, as every other refusal in the UI does.
- The field saved the text exactly as typed, and the helm accepts a path with a trailing space. A path pasted with a
  stray trailing space would then make the first checkout create a folder whose name ends in a space, which the preview
  does not show. The field now trims surrounding whitespace before saving, so an all-space entry clears the folder. No
  new test covers the trimming; the existing Settings save and clear cases pass.

#### Things to know

- Possible follow-up before the release notes point people to Settings: the user docs (the website's "start a session"
  page and `docs/github-checkouts.md`) still say to create the folder by hand on each host and run
  `farhelm helm checkout-config set-root`, and quote a "no checkout root is configured" message that no longer exists.
  The plan left the docs page out on purpose.
- Left as they are:
  - some refusals the field can show still say "checkout root" while the UI says "checkout folder";
  - Enter in the field neither launches (intended) nor saves;
  - the Settings field's note borrows a launcher style;
  - the settings switches' styling is scoped by the dialog's accessible name, so renaming that dialog would silently
    drop it;
  - a folder created at first checkout stays, empty, if a later check refuses the checkout, as the plan accepted.
- The review checked the folder-path handling closely and found nothing: `..` after a missing directory and dangling
  links are refused, creation uses the path the preview showed and re-checks it, and the browser never sees per-host
  folders or post-clone commands.

#### Checks

These two plans were landed together, ui-correctness-fixes first with the checkout-folder stack on top, on main after
browser-terminal-test-oracles merged.

- Run now, on the two stacked, after the fixes below: `cargo fmt --all -- --check`, `dprint check`, the changelog lint,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor, helm,
  UI, protocol and CLI unit tests with the GitHub-checkout end-to-end tests, through the recorder with pinned tmux 3.7c,
  four slots and no retries (run `7b43b961`, 2891 of 2891); the UI JavaScript tests (230 of 230); on Chromium and WebKit
  with one worker and no retries the approval-layout, feedback, github-checkout-composer, settings, templates,
  listing-mutation-fence, notifications, provisioning, restart-with, terminal-font, terminal-links,
  terminal-replay-rename and mouse-modes specs (run `bf94c1ca`, 374 passed); and the sidebar's launcher Tab-order test
  on both engines (run `aeda93f9`, 2 passed).
- The terminal-links, terminal-replay-rename and mouse-modes specs were run on top of browser-terminal-test-oracles,
  which changed the link tests that ui-correctness-fixes' link-selection change has to pass.
- An earlier run of the same Rust selection, before the test fix below, stopped at that test's failure; it is not
  counted as a pass.

Nothing in the report above was made untrue by the landing.
