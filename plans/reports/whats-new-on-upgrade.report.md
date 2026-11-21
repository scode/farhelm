# Release highlights after an upgrade

## What this was about

After an upgrade, Farhelm left users to find the full release notes themselves. An upgrade that jumped across several
stable releases made it easy to miss the changes in between. The maintainer chose a text dialog with links to the
screenshots on the website and an opt-out that can be reversed from Settings.

**What's new in Farhelm** opens in the installed Mac app after Welcome and Announcements. It groups the available
highlights through the running release. Each release has its date and categories, and entries with screenshots link to
them. The total of the other changes stays visible beside the actions, including the link to the full notes.

## Things you should know

The helm remembers the last release shown or skipped. A newly created helm starts at its own release. An existing helm
with no saved release offers the running release once. The dialog works with automatic updates off. **Don't show again**
turns off these highlights, and **Show what's new after updates** in Settings → App turns them back on for the next
upgrade.

The app reads finalized notes from the website. It retains the saved release when the site is unavailable or has yet to
publish the running release, so the next launch can try again. A slow response yields to the user's current work and to
a prompt that already has focus.

The stable release procedure waits for the maintainer's website deployment after the release is cleared to go out. The
live check must find that release's finalized highlights before the procedure hands over the command that moves
`/latest`. Validation here used the locally built feed.

## Open questions and possible follow-ups

The installed Mac path and VoiceOver still need native release validation. The desktop tests exercise the startup
sequence and accessibility tree with a representative feed. The Linux captures cover light and dark appearances.

## Pull requests

- Publish the notes feed and screenshot anchors. [#4419](https://github.com/scode/farhelm/pull/4419/changes)
- Check the notes before stable promotion. [#4421](https://github.com/scode/farhelm/pull/4421/changes)
- Keep the last release and the opt-out on the helm. [#4423](https://github.com/scode/farhelm/pull/4423/changes)
- Add the installed app's feed lookup and release selection. [#4426](https://github.com/scode/farhelm/pull/4426/changes)
- Show the upgrade dialog and add the Settings switch. [#4431](https://github.com/scode/farhelm/pull/4431/changes)

## Checks

- The three generator tests in `website/scripts/release-notes.test.mjs` and the production Astro build passed on the
  code from `5aae8883`. The built feed listed both finalized releases, and eight screenshot links resolved to distinct
  anchors. The release procedure's feed check accepted a version present in the built artifact and refused one absent.
  Both patches are identical after restacking.
- The helm's store, preferences, checkout configuration and UI channel passed all 290 selected nextest cases on
  `d21255f1` plus the preference patch in `422e88b6-5d84-4211-8fb5-854b63054cf8`. All four desktop selection and
  screenshot URL cases passed on the lookup code from `c866c88e` in `0f06235f-964e-4d4a-955c-ac35f7be0f59`. Later
  changes on main concern separate paths, and the restack preserves both patches.
- All 28 selected What’s New, Welcome, Announcements and Settings tests passed on `5f653018` in
  `2e7cbe10-989e-4ece-b096-5967c355de6d`. The earlier 110-test run `05c8e8a2-0249-494c-bc81-3e21a4a71ac7` used
  `12f23586` plus the early dialog patch and covered Help and Settings. The 45-test run
  `29665870-8c36-4742-a468-5f6ae9d13480` covered the frame and accessibility on `68c0ea6d`.
- The isolated Linux startup smoke step 53 passed in `a97cab6a-d8ee-4706-8b3b-e5654eaaded2` using the app built from
  `68c0ea6d`. The two full smoke attempts did not finish. The first was interrupted during the build. On retry, headless
  Mutter skipped clipboard coverage, then the run was interrupted in the hosts scenarios.
- Inspected both new upgrade dialog pictures and both changed Settings App pictures. All 302 Linux captures matched
  their references on `5f653018` with `python3 -B scripts/check-gui-goldens.py` in
  `f1812dd0-2589-4916-8aa9-531efaa80eb1`. The final restack also carries the independently checked Help and session menu
  pictures from the menu hints change. The two changes use different scenes. Added that plan's new menu scene to the
  catalog the comparison page uses. All 47 screenshot-batch tests passed on the combined catalog in
  `60b39564-9088-4340-9057-96ecc5def9c2`.
- Formatting, changelog and instruction checks passed. The instruction checker reported one existing archive citation
  warning. The test sleep check found no missing rationales. Clippy passed for the GUI and desktop app with the
  automation bridge enabled on `5f653018`. The focused checks cover the changed behavior. Full workspace and hosted CI
  runs were skipped.

## Review

Each draft passed its independent source gate, and the draft messages went through cold reads. Review caught historical
database fixtures that needed updating, a gap between the URL checked and the one opened, and two dialog issues. The app
now opens the validated URL, leaves an active prompt in focus, and exposes the release text and total to assistive
technology.

### Landing

#### What landed with it

Nothing else landed in this round, and main had gained nothing since the executor last rebased this plan. An independent
reviewer and I read the five PRs. In the release instructions, no changed text has an agent sign, publish, deploy the
website on its own or move `/latest`; the new wait sends you the urgent notification, and the feed check only reads. The
new saved release and opt-out exist for both new and upgraded helms. The app's download is fixed to
`https://farhelm.io/whats-new.json`, HTTPS only with no redirects, limited to 10 seconds and 2 MiB, run off the main
thread, sends nothing but the request itself, and happens only in an installed stable Mac app with highlights on and a
newer release running. If the request fails, nothing is shown and the app tries again at the next launch.

#### Fixes made while landing

One went in as a sixth PR, [#4432](https://github.com/scode/farhelm/pull/4432/changes):

- The workspace run caught one failing test, which checks that every Settings search result points at something the
  dialog draws. Its fake updater offered no upgrade highlights, so the new **Show what's new after updates** switch was
  never drawn. Giving the fake highlights then exposed a real ordering bug: search listed the switch before the App
  section's update controls, which the dialog draws first, while search results are meant to follow the drawing order.
  Search now lists it after them, and the test gives its fake updater highlights. The executor's report lists only GUI
  runs restricted to selected tests, which did not include this one.
- CI's website job now also runs the release-notes generator's test, which only local runs ran before, and the list of
  checks agents choose from says so.

#### Checks

The plan changes the docs website (the new feed and its generator, a dependency), the stable release instructions, the
helm's database (to version 94), the app's channel to the helm, the desktop app, its GUI and reference pictures, SPEC.md
and SPEC_impl.md. The landing ran the whole workspace suite, the desktop app's tests, the app's end-to-end smoke test
and the lint runs (clippy) on a separate arm64 Linux test machine, and the website build and its new test, the
screenshot comparisons and the small checks locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine (recorder run
  `8e8c2629-83be-4b7e-b9c7-e2854e082b92`): 8194 of 8195 passed. One failed: the GUI test that checks every Settings
  search result points at something the dialog draws (see the landing fix). No test reported skipping itself for a
  missing machine capability.
- `cargo nextest run -p farhelm-desktop`, on the test machine (recorder run `b57c8ef5-888f-4a3d-b712-50339b4eb1a8`): all
  184 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (recorder run `a229a185-20fb-43e4-8969-04678eec054b`):
  every step passed but paste, which skips on Linux as it always does. Its steps include a run with the start dialogs
  on.
- `cargo clippy --all-targets -- -D warnings` and the desktop app's own two clippy runs (with and without
  `--features agent`), on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `cba7cc7e-3bd8-4fdd-a696-3ab1247efe27`): passed, with the plan's new dialog and Settings pictures.
- The website build (`bun install --frozen-lockfile && bun run build`) and
  `node --test website/scripts/release-notes.test.mjs` (all 3 passed), locally.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `python3 scripts/check-agents-md.py`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After the landing fix (#4432), which changes the GUI's Settings search, that test, the CI workflow and one line of the
test inventory:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `759541fb-207b-4d47-b546-b07bea619ddb`): all
  3231 passed. A first attempt that fixed only the test (recorder run `ba167955-0c23-484f-a53d-a17ed2d690b6`) still
  failed it, on the search order, which led to the code fix.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed. `cargo fmt --all -- --check`, `dprint check`
  and `python3 scripts/check-agents-md.py`, locally: passed.

The other runs above still apply to the fix, which changes nothing outside the GUI's Settings search, CI and the
inventory. Skipped: Mac runs and VoiceOver, which the report leaves to native release validation.

#### Things you should know that the report does not say

- A release can have no highlights at all, with every change only in the collapsed full list, which is a likely shape
  for a patch release. Upgrading to one opens the dialog with only the version and "N additional changes not
  highlighted". A small follow-up could record such a release as seen without opening the dialog, the way it already
  does when highlights are turned off.
- The new promotion step says you deploy the website yourself, while the separate instructions for website deploys still
  let an agent deploy when you ask it to. So if you tell the release agent "you deploy it", it could read either rule as
  applying. Neither reading lets an agent move `/latest`, but one sentence in the promotion step saying whether an agent
  may deploy when you ask would remove the doubt.
- The changelog fragment promises highlights "from the stable releases you missed". On the first upgrade from a helm
  that never recorded a release, only the running release's highlights show, and nothing shows until the website is
  deployed. The screenshot-links fragment says release notes "now have direct links to entries with screenshots"; the
  change adds link targets (anchors) that such links can point at, not visible links. Release curation can fix both.
- Whether farhelm.io serves the feed without a redirect is untested until the first deploy. If it redirected, the app
  would show nothing and try again at each launch, and step 8's feed check would catch it before promotion.
- The plan also adds the screenshot-list entry for the open session row menu picture that the menu shortcut plan added
  in the previous round without one.

#### What you should still do

The report's native check stands: on the installed Mac app, after an upgrade, check the dialog appears after Welcome and
Announcements and that VoiceOver reads it.
