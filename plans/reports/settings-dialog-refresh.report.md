## What this was about

Settings felt flat, and long pages could put Save and other actions below the fold. The section list now stays beside
the page and uses one icon color per group. The page heading explains what the settings affect and stays visible above
the scrolling content. Related settings share cards. Appearance previews both themes, and you can play a notification
sound from its row.

## Things you should know

Editing a form marks its field and section with dots. Save and Discard stay in the footer. Close asks once about unsaved
changes across the dialog and names the affected pages, including Checkouts when it has a draft. If a save is refused,
Settings stays open and shows the affected page. The key fields on the Tensorlake and Credentials pages share one
account. If they contain different replacement keys, Settings opens Credentials for you to choose which key to save. An
open credential draft is preserved while you resolve that key.

The scope sits next to the page title so the description has enough room at the 720 × 560 minimum. App keeps its Mac
scope, and the upgrade-highlights switch says that its choice applies across the helm.

The Tensorlake section uses the existing key icon. The recipe and credential footer notes use the form names, which fit
in the pinned row at small window sizes. A Checkouts draft stays marked when you switch sections. In the close
screenshot, the prompt overlays the Checkouts indicator in the section list.

The Appearance choices use the shared radio-card borders. The ntfy topic caution uses an amber mark, side rule and
background with the usual help text color. Send test sits at the card's right edge with its note beside it. The recipe
Delete warning wraps above Cancel and Delete at the minimum size.

## Open questions and possible follow-ups

The Settings guide reflects the dialog. The Manage hosts guide's `manage-hosts/app-settings` picture still shows the
previous layout and is due for the next docs screenshot refresh. The published release-notes screenshot records the
layout of that release.

A scrolled Credentials overview would show the lower lists in a future screenshot batch. The current overview capture
shows a saved GitHub token with Edit. The GUI tests cover the lower list headings, Add actions and empty-list wording.
The editor keeps its existing Save rule for a new recipe, and the two delete questions use the app's existing footer and
danger treatments.

## Pull requests

- Grouped navigation and a pinned page heading. [#4434](https://github.com/scode/farhelm/pull/4434/changes)
- Pinned Save and Discard, draft dots, and one question on close.
  [#4435](https://github.com/scode/farhelm/pull/4435/changes)
- Cards for the switch pages, appearance previews, sound previews, and grouped search.
  [#4437](https://github.com/scode/farhelm/pull/4437/changes)
- List cards, recipe and credential editors, and the revised Settings guide.
  [#4439](https://github.com/scode/farhelm/pull/4439/changes)

## Checks

- `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::|^components::tests::(radio|checkbox|dialog|button)|^add_host::|^host_settings::|^hosts::add::|^launcher::checkout/)'`
  passed all 358 selected tests at `7ab678ba` (recorder `76667fb0-b293-43f4-b188-37ef8253b4d3`). It covers the complete
  Settings change, the shared controls, Add host and the launcher's checkout form.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings` passed on the same code. The final captions and screenshot
  references do not change what compiled.
- `python3 scripts/check-gui-goldens.py --update` rendered the complete Linux set at each step. The final render has 328
  pictures, including 62 Settings pictures in both themes. Only Settings references changed. Frame recorder
  `f1d0cd37-75bf-4bca-9fae-81427620fa2e`. Pinned footer recorder `d031a263-83d3-4599-9a80-fd7e5973b726`. Switch pages
  recorder `8e9f1d76-5bde-4d7a-8632-2c41fb03d387`. List and editor pages recorder
  `1e7b16b3-701c-4075-b49f-ada9a1a930c1`.
- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py`, and `python3 scripts/check-agents-md.py` passed. The instruction checker
  reported an existing stale archive heading reference.
- `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 cases against the final screenshot manifest (recorder
  `9a2270f1-dde7-4e92-beb2-979774eedc3c`).
- `bun install --frozen-lockfile && bun run build` built the guide and verified its links. The guide source stayed the
  same afterwards.
- The Linux captures exercise the App and Terminal layouts with staged Mac controls. Native macOS, installer and
  full-workspace checks were deferred because the work is in Settings and the focused checks cover its shared GUI
  controls.

## Review gates

All four drafts passed the independent Sol high source and Opus high visual reviews after corrections. The final visual
pass reviewed all 62 Settings pictures against the agreed mockups and the written design. It verified the full recipe
deletion warning and the pinned Checkouts actions at 720 × 560. The other application screenshots matched their previous
references by content.
