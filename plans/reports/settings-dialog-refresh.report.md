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

### Landing

#### What landed with it

Nothing else landed in this round, and main had gained nothing but queue bookkeeping since the executor based the plan.
An independent reviewer and I read the four PRs against each other and against the specs. The changes to the shared
controls (the dialog frame, buttons, check boxes, radio groups and text fields) only take effect where Settings asks for
them, so the launcher, host settings and Add host look and behave as before. The checkout folder and post-clone command
fields are also used by host settings and the launcher's checkout setup; their Save moves to the footer only inside
Settings.

The plan still in flight, launcher-command-field, adds the same card color to the theme. It will meet a textual conflict
there when it lands; both plans expect that, and its landing keeps a single definition of the color.

#### Fixes made while landing

All of them went in as a fifth PR, [#4440](https://github.com/scode/farhelm/pull/4440/changes), which changes no
behavior. Closing Settings now asks about every unsaved draft, but several texts still said closing throws unsaved text
away:

- The Tensorlake sandboxes docs page, in its section on saving named credentials. It now says Settings asks whether to
  save a typed value first.
- The recipe editor's code comments, which quoted a spec sentence this plan removed and still said the recipe editor
  asks its own question when Settings closes. Now the Settings dialog asks one question about every draft; the recipe
  editor still asks its own question only when Add host, which embeds it, is closed.
- Four test descriptions, and the name of one credential test that no longer checks closing with a draft.
- A code comment that still gave the section list's old row height. The plan made the rows 28 pixels high instead of 34,
  to fit the new colored icon tiles; only the comment was stale.

#### Checks

The plan changes only the desktop app's GUI crate (Settings, the shared controls and the theme), its reference pictures,
SPEC.md, SPEC_impl.md, the Settings guide and the developers' list of how the new app's Settings differs from the old
one. The landing ran the GUI crate's tests, the app's end-to-end smoke test and the lint runs (clippy) on a separate
arm64 Linux test machine, and the screenshot comparisons, the website build and the small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `bfe3fa96-895c-4e72-a07d-c012e23df5a2`): all
  3238 passed; the two skipped are the screenshot comparison, run separately below, and a generator that runs only by
  hand.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (recorder run `548beb54-871e-4da3-8915-36de8a9a4b37`):
  every step passed but paste, which skips on Linux as it always does. Its steps open Settings from the gear and close
  it with Escape.
- `cargo clippy --all-targets -- -D warnings` and the desktop app's own two clippy runs (with and without
  `--features agent`), on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `cd32e6ad-8db4-4a2d-952e-625cb2271cc3`): passed, with the plan's new Settings pictures.
- The website build (`bun install --frozen-lockfile && bun run build`), with its link check, locally: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `python3 scripts/check-agents-md.py`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fix (#4440), which changes comments, test descriptions, one test's name, a docs sentence and that
developers' list:

- `cargo nextest run -p farhelm-gui -E 'test(/^settings::/)'`, on the test machine (recorder run
  `0ef52b8b-5588-4961-8dda-fe69680d061a`): all 229 passed, the renamed test among them, and
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings` passed.
- The website build, `cargo fmt --all -- --check`, `dprint check` and `python3 scripts/check-agents-md.py`, locally:
  passed.

The other runs above still apply to the fix, which changes no code that runs. Skipped: the workspace suite and the
desktop app's own tests, since nothing outside the GUI crate changed and the lint runs compiled the desktop app against
it; CI, which runs only on demand; and Mac runs, which the report leaves to native validation.

#### Things you should know that the report does not say

- A small bug with no data lost: if Send test (ntfy alerts), or the Tensorlake key's Test or Clear, is still running
  when you close Settings and choose Save, that form's save is refused without a message. The question disappears and
  Settings stays on that page, with your draft intact. A follow-up could treat a running test as a pending save in the
  question, or show the usual "Not saved: another operation was in progress. Try again." line.
- The Tensorlake API key can be typed on two pages, Tensorlake and Credentials, which save the same key. When both hold
  different typed keys, closing with Save refuses to pick one and shows "Two different Tensorlake keys are typed. Save
  the one you want first." (the plan's wording). But once you save one from its page, the other page's typed key is
  still there, and the next close question's Save writes it over the key you just chose. I count that as a bug worth a
  small follow-up: either drop the other page's draft once one key is saved, or say "Save the one you want and discard
  the other."
- While that two-keys message shows, Settings opens the Credentials page's key form even if a credential editor was open
  there. The executor added a "Resume editing credential" button, not in the mockup, that brings the editor back with
  its draft. It works.
- A saved credential's Delete is now a quiet red button in the footer, next to Cancel and Save, and deletes with one
  click. It never asked for confirmation (the report's "two delete questions" are a recipe's and a leftover
  container's), but it used to sit apart as a solid red button, so a misclick is now easier.
- No test closes Settings with a typed credential secret; the close question's tests cover the ntfy and checkout forms,
  the recipe editor and the two Tensorlake keys.
- Asking to close with a single unsaved form switches to that form's page (and clears a search), even if you then choose
  Keep editing.
- The changelog has four fragments for this one feature, which release curation can merge.

#### What you should still do

Nobody has run this on a Mac yet; the report's checks and these ran on Linux, where the App page's Mac-only controls
were drawn from stand-ins and no sound can play. On the first Mac build that has this change (a dev build or the next
release), open Settings once: check the App page and press a play button beside the sounds.
