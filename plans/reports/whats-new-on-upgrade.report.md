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
