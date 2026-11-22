# Launching on a suspended host

## What this was about

Tensorlake suspends idle sandboxes, and users can return to a stopped container. The desktop launcher used to send the
user to Hosts to bring it back before it would start a session. The agreed behavior was for the launch itself to wake
the host and wait for a fresh connection, including when the session needs a new GitHub checkout.

## Things you should know

The launcher hides after the press and a small status pill appears at the bottom of the window. The user can keep
working in other sessions. Once the host connects, a fresh GitHub checkout gets its folder preview on that connection. A
typed name still asks for its exact folder. A template's base name can choose a numbered folder. If another launch takes
the chosen folder after preview, the launcher shows the new path. The user presses Launch again to accept it.

In that window, New remains busy. Suspend, Resume and Stop and suspend are unavailable on the destination host. Host
setup and host registry changes also wait. Restart with and Replace with hold their source session through preparation.
Resume can hold up other requests from the same window for up to 20 seconds, as agreed while the separate work on
parallel requests is pending.

Busy retries and connection waiting share one 60-second budget, paused during this launch's accepted Resume. On expiry a
refusal says the session was not launched. The draft comes back immediately if the user is still waiting for it.
Otherwise a notice offers the draft. A host arriving after expiry cannot start that session. A checkout preview can
finish after the minute once the host has connected.

## Open questions and possible follow-ups

An open launcher can still launch from a recent checkout by pressing Enter on that setup, if it read the history before
suspension and the installation still matches. Opening a launcher while the host is already suspended still works from a
repository choice or template. Showing recent checkouts on that first opening is a possible follow-up because the helm
currently needs a connection to read them. The historical browser test simulated suspension in its window while the helm
could still read history from the connected host.

## Pull requests

- Disable conflicting host controls during an existing-host launch.
  [#4564](https://github.com/scode/farhelm/pull/4564/changes)
- Wake an existing host before launching into a folder. [#4568](https://github.com/scode/farhelm/pull/4568/changes)
- Wake an existing host before previewing a fresh checkout. [#4573](https://github.com/scode/farhelm/pull/4573/changes)

## Checks run, reused and skipped

- Recorded Linux `cargo test --locked -p farhelm-gui --lib` selections for all three drafts. The Hosts, session
  environment and prompt selection covered #4564's Rust changes at `6645703496fd` and passed 94 tests (run
  `39937253-3edf-44ec-bdaa-71c5cca1f011`). The wake, launch requests, pending launches, template dispatch and session
  header selection covered #4568's Rust changes at `7e84831ebe2d` and passed 96 tests (run
  `ebea15fb-f2aa-4aa0-8670-44643630d1ff`).
- For #4573, the checkout, folder defaults, recent setups, launcher search, wake and template dispatch selection passed
  178 tests and found one test-fixture failure (`d69cc385-e0fc-472d-a1e2-f911b776faed`, with the checkout changes in the
  working tree on `7e84831ebe2d` before the last fixture correction). Reused the passing results for tests unaffected by
  the fixture correction. Reran all eight affected checkout wake tests with the corrected fixture in `4807af3b1455`,
  including the marked and numbered shortcuts, launching from a recent checkout and both sides of the time limit. All
  eight passed (run `a1d3cebe-6314-4d6e-9df1-dd21650ab5e2`).
- `python3 -B scripts/check-gui-goldens.py --update` rendered the full Linux catalog for each draft. The selected
  comparisons matched both appearances. The Stop confirmation at `6645703496fd` matched in run
  `d3889b6f-8f1d-4249-9515-64711f9a2eba`. The preparation pill at `7e84831ebe2d` matched in run
  `df65ca70-0b35-4f1d-ba2b-553e94759bba`. The allocation notice at `4807af3b1455` matched in run
  `b0da26f5-d1a8-45e0-808d-449faa2cb687`. Formatting, changelog and test-delay checks passed.
- Main gained the shell-tab fix for shortened session lists. The rebase preserved both sets of screenshot references.
  The combined `SceneList.test_the_committed_scene_list_is_well_formed` check passed (run
  `8075e6b0-5fa2-431c-a06c-a116b15a1cd8`). The rebased drafts have the same diffs as the reviewed and tested drafts.
  Shell-tab retention reacts to an omitted session row, a different event from the host reconnecting during a launch.
  The earlier desktop tests and render checks still cover the changed behavior.
- Skipped the full workspace suite and a real provider wake. The change is in the desktop launcher. The fake helm
  exercises the request, ordered replies and pushed host states. Live provider behavior was outside this check.

## Review gate

Each draft passed the separate Sol high source review after its findings were addressed. The Stop confirmation,
preparation pill and checkout allocation notice each passed the Opus 5.5 high visual review in both appearances. The
stack rebase preserved the reviewed changes.
