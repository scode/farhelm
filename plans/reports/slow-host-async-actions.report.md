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

### Landing

#### What landed with it

The plan's three PRs (#4623, #4626, #4631) landed first in this round, with one landing fix (#4633), followed by the
change that makes Set up and launch the primary button of the launcher's hook setup question. Nothing that landed on
main since the plan was written touches how the desktop app and the helm exchange requests, and the rebase applied
cleanly.

An independent reviewer read the change closely and found no blocker:

- No protocol version change is needed. The desktop app only ever talks to the helm built into it, so the two are always
  the same build; even if they differed, an older helm would run everything in order and an older app would accept the
  replies.
- Nothing that relied on order lost it. Wherever an action that may now be answered out of order could have raced
  another, something already waits for the reply first: Stop or Delete on several selected sessions goes out one host at
  a time, a launch waits for its own wake and a fresh connection, and actions on the same session wait for each other.
  Everything else still runs in order.
- No reply can be lost or answered twice, on either side, and the test changes do not weaken anything they checked.

#### Fixes made while landing

- #4633 corrects a comment in the launcher's tests that said the stand-in helm they use answers every request in the
  order it was sent. That stopped being true for the session row menu's Stop and Delete, which those tests use.

#### Checks

The round made two changes: how the desktop app and its helm exchange requests, and which button of one launcher
question is highlighted. So the landing ran the Rust workspace suite, the desktop app's tests, the lint runs, the
end-to-end smoke test (which drives the real app against its helm) and every reference screenshot, once, on the two
plans stacked together. The worker runs were on a separate arm64 Linux test worker with a systemd user manager and SSH
to itself; the rest ran locally on x86_64 Linux.

On the two plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `f0ba02fa-1ab5-4a65-a5e6-d5c75650ddc8`): all 8363 passed, and none printed a runtime `SKIPPED`. The 86 tests marked
  ignored did not run, as on every run.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and both desktop app
  clippy runs (with and without `--features agent`), on the test worker: passed.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `86d59245-c004-464c-94dd-3047a8a10644`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `b5ae6ec4-4108-4cda-bc05-714072e01add`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `333b9da3-3dca-4281-bcb8-6bb71cdfcd82`):
  every reference screenshot matched, including the hook setup question's updated light and dark pictures.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the two landing fixes (#4633 and #4634), which change only comments and the desktop window's design notes:
`cargo fmt --all -- --check`, `dprint check` and `python -B scripts/check-test-sleeps.py`, locally: passed. Everything
above still applies.

Skipped: macOS, since nothing in the round builds only on a Mac; CI, since the checks above already cover the round and
it runs only on demand.

#### Things you should know that the report does not say

- A session's Delete can now be answered while a launch on the same host is still waiting, which used to be impossible.
  The launcher's existing rules for what to select after a Delete cover it, and the reviewer saw nothing wrong, but no
  test checks that order; the existing test only covers the old one. A follow-up test would close it.
- About half a dozen other test comments still say the stand-in helm answers everything in order. It fails loudly if a
  test relies on the wrong order, so these can only mislead a reader; they can be fixed the next time those tests are
  touched.
- SPEC.md still describes the window and the helm as joined by an ordered stream; the exception for these actions is
  written down only in SPEC_impl.md. You approved that split; this note is the only place that raises whether SPEC.md
  should mention it, which fits the architecture review this plan added to TODO.md.
