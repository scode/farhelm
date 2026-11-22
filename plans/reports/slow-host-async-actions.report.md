# Slow hosts and independent desktop actions

## What this was about

While the desktop app waited for one host, later actions on every host had to wait, and live status updates also paused.
The maintainer chose three groups to move first. This stack keeps live updates flowing and lets session Stop and Delete,
environment Suspend, Resume, Stop and suspend, Delete from a session menu and Retry-delete, and Checkout Trash listings
and per-host deletes run independently. Commands that can depend on earlier work, such as a launch using a template the
user just saved, still run in order.

## Things you should know

The app can show the host's changing phase before Suspend or Resume answers. Launch still waits for the result of its
own wake request and for a fresh connection before it submits. A conflict on an environment can report busy sooner when
the helm receives both requests while the first is still running.

Trash still marks an accepted row Removing while its refresh is pending. If the newer listing arrives first and there is
no other pending read for that host, the late delete reply leaves that listing alone. The app waits for the host to
report a Trash change after a delete timeout. A failed listing keeps the last reported removal state and stops automatic
rechecks.

## Open questions and possible follow-ups

The next design review should decide which remaining actions can finish beside a slow host and which depend on earlier
work. It should also review when a long wait resets the window, since a reset reloads live state and drops pending
replies. Credential replacement remains ordered and has one 20-second wait across hosts, with up to four deliveries in
parallel. Launcher reads and Launch, session Rename, Restart, Replace, tabs and tunnels, and host management and
settings remain ordered. This includes Delete sandbox or container in the Hosts panel.

Opening Trash as a fleet connects can send two reads per connected host in quick succession. Two failed-refresh
orderings also remain. If the first refresh fails before the delete reply arrives, Trash can show the read error without
showing the newly accepted removal. If one refresh already showed cleanup finished, another read is pending when the old
delete reply arrives, and that pending read fails, the Trash icon may keep showing active removal until a normal
refresh. The architecture-review TODO records both cases and the existing choices about failed reads and delete
timeouts. Both display cases are deferred to that review.

## Pull requests

The three drafts are stacked in the order below for landing. The shared session-action helpers were already on main at
the final validation base. [Shared session-action helpers](https://github.com/scode/farhelm/pull/4610/changes)

- Live status updates during a host wait. [#4623](https://github.com/scode/farhelm/pull/4623/changes)
- Independent commands on the desktop channel, with ordering preserved by default.
  [#4626](https://github.com/scode/farhelm/pull/4626/changes)
- Move the selected desktop actions and preserve newer Trash state.
  [#4631](https://github.com/scode/farhelm/pull/4631/changes)

## Checks run, reused and skipped

- On the final tree, recorded `cargo nextest run -p farhelm-gui --lib` with a focused filter for the operation store,
  Trash, session lifecycle, launcher wake and credentials. All 179 tests passed with no runtime skips (run
  `80a431cb-2e32-4e71-ab5e-9f715296a2da`). The Checkout Trash button and dialog matched in both themes with
  `python3 scripts/check-gui-goldens.py --scene 'checkout-trash-*' --scene 'trash-button-*' --jobs 2` (run
  `179c920f-9ebe-43c9-8e2f-56df16237289`). Strict GUI `cargo clippy --all-targets -- -D warnings`, Rust and Markdown
  formatting, changelog and whitespace checks, and all 309 test-delay rationales passed. The instruction check had five
  pre-existing reference warnings and no errors.
- Reused 105 focused channel, protocol, client, fake and launcher tests (run `d1b0ffd1-5fec-4eaf-b841-dd651698ee95`) and
  strict four-package Clippy from revision `57dcbd3dc6ce`. The rebases kept the transport changes and the last draft
  changed only helm route comments. The final GUI run covers interactions with the shared session-action helpers that
  landed on main. The recorded `cargo nextest run --lib` selected the channel in the protocol, helm and helm-client
  packages, the fake and launcher/wake cases, with strict Clippy covering those packages and the GUI. Also reused the
  77-test live-push run from `a191a5cc81fe` (helm and GUI channel, launcher, wake and suspend reconnect tests, run
  `9d3f53cf-72ca-4bda-a94e-4426127585b3`). Later transport tests still included the live-push regression, and the final
  GUI run rechecked the affected launcher on current main.
- Two GUI fixtures initially failed to compile after the fake command gained its lane field (run
  `3de118b0-b2b9-4e87-b8e2-72a49f209d22`). Corrected the patterns and verified them in the 105-test run. Retained failed
  GUI runs `c2e66183-fb09-4bd9-a893-ce82ca63ed3b` and `3b6f68de-01c5-4182-9747-415a6527ea03`. The latter two exposed a
  wrong removal flag in the new fixture and an assertion before the dialog's click guard expired. The corrected focused
  selection passed 58 tests before the final rebase (run `02fc6055-c369-481b-a234-06459a540bd4`).
- Skipped the full workspace battery and unrelated golden scenes. The focused checks exercise the modified channel
  contracts and the affected GUI paths.

## Review gate

Each draft passed the plan's single adversarial source review and a separate cold read of its commit wording. The
reviews led to fair polling across both lanes, a shared completion flush for channel-local query replies, and clearer
test premises and channel documentation. The additional Opus design consultation kept the existing Removing display
without adding read-ordering state. The remaining Trash cases are listed above.
