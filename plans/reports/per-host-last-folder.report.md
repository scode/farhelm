# Report: the launcher remembers each host's last folder

## What this was about

Your TODO entry asked the session launcher to remember the most recently used folder separately for each host, with a
new host starting in its home directory. You wrote that you kept having to switch between macOS and Linux paths: New
started at the selected session's folder when that session was on the chosen host and otherwise at `~`, and changing the
host never changed the folder, so a Mac path stayed in the field after you picked a Linux host.

On the planning page you decided:

- the memory comes from the launcher's existing recent setups, with no new storage, so it counts agent launches you made
  from the GUI (New, quick launch, Clone and Replace with) and not agent-created sessions, command launches, or launches
  into a new managed checkout;
- the selected session's folder still wins on its own host, then the host's last folder, then `~`;
- changing the host re-seeds the folder unless you typed or picked one in that draft;
- a remembered folder that no longer exists is used anyway, and the launch is refused as for any missing folder;
- the reviewer is gpt-6.1-sol at high effort.

The plan built one PR with exactly that behavior. Quick launch uses the same default. The spec, the implementation notes
and the Start a session docs page describe it, and the TODO entry is removed.

## Things you should know

- **Launch waits for the host's recent setups.** Until the launcher has read the chosen host's recent setups, Launch is
  refused for a folder you have not typed or picked ("wait for this host's last folder before launching"). Without this,
  a launch right after a host switch could still send the previous host's folder, which is the problem being fixed.
  Normally the wait is a fraction of a second.
- **If the recent setups cannot be read, the folder falls back to `~`.** A quick launch started from its keyboard
  shortcut, which normally launches without showing the launcher, then shows the launcher with an explanation instead of
  launching into a guess. If the read succeeds later, the open launcher does not switch to the remembered folder; this
  keeps Launch available during an outage, at the cost of staying on `~` for that opening.
- **A refresh never moves an open draft's folder.** The folder is decided once per opening or host change, from the
  first reply for that host. Another window's launch on the same host does not change a launcher that is already open.
- **Clone and Replace with keep their source's folder** until you change the host yourself. Their automatic binding to
  the source host does not count as a host change.
- **The browser suite now depends on recent setups.** Browser tests share a helm, and earlier tests' launches become
  recent setups, so New can open at an earlier test's folder instead of `~`. Two tests that assumed `~` were found; they
  now answer the launcher's recent-setups read with an empty list, so they no longer depend on earlier tests. The specs
  covering New, Clone, Replace with, quick launch, the host picker and YOLO prompts all pass. A spec outside those that
  silently assumes `~` could start depending on test order; see the follow-ups.
- **The review took eleven rounds.** The reviewer kept finding real but increasingly narrow interactions between the
  folder default and the launcher's other machinery: Clone's host binding, the key that lets a retried launch reconcile
  with one whose reply was lost, the check that a folder inherited from the selected session still belongs to that
  host's current installation, and above all permission Undo (the banner that undoes a YOLO-switching agent change).
  Each finding was fixed. The final round found nothing. A separate scope review found no mechanism the plan does not
  need. The result is a self-contained step with its own unit tests rather than changes spread through the launcher, but
  it is the most intricate part of the change.

## Open questions and possible follow-ups

- **Permission Undo and the folder default have no browser test.** The Undo interactions the reviewer found are covered
  by unit tests of the folder-default step, but no browser test drives the permission Undo banner through a host change.
  Recommendation: leave it unless permission Undo changes; the unit tests pin each case, and the existing permission
  Undo browser tests still pass.
- **A recovered recent-setups read does not re-seed** an open launcher that already fell back to `~` (see above); an
  ordinarily opened launcher shows no sign of the fallback beyond the folder itself. Recommendation: leave it; the
  alternative refuses Launch for as long as the read keeps failing.
- **Browser specs that assume New opens at `~`** without saying so could become order-dependent. Recommendation: fix any
  that turn up the same way the two found here were (answer the recent-setups read with an empty list); a sweep of the
  whole browser suite for this is not worth it on its own.

## The PRs

- #2669 — default the launcher's folder to each host's last one

## Checks

Run on the final code, after rebasing onto the current main (which by then included the session-view fixes):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown -- -D warnings`: all clean.
- The launcher's unit tests through the test recorder, `cargo nextest run -p farhelm-ui --lib -E 'test(create_form::)'`:
  93/93 (run `d7204c89-7b91-4737-938b-2e2a8e6826fb`).
- Browser specs on Chromium and WebKit through the recorder: yolo-guard, replace, terminal, destination-authority and
  quick-launch, 308/308 (run `f2ddc56f-5c76-4f1e-9f5c-64e1f78af00f`).

Reused, from the same branch before the rebase:

- sidebar, terminal-create-idempotency, clone and permission-switch, which make up the rest of the 586/586 run
  `d85934cd-aa77-439e-8b59-e51e2ed9d167`. That run predates two small fixes to what permission Undo restores; clone and
  terminal-create-idempotency never use permission Undo, and permission-switch and sidebar, which do, passed again on
  the fixed code, 256/256 (run `20ac46a5-1e14-4497-b973-a21e4d4a157b`). The rebase brought in the session-view fixes;
  they touch the launcher's YOLO question and recent-row hint, which yolo-guard, replace and terminal cover and which
  were rerun above, and the closing of session menus after a failed list read, which does not involve the launcher, so
  sidebar was not rerun after the rebase.
- `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format` and `dprint check` on the
  changed Markdown: all clean.

Skipped: the docs website build (the docs change is one prose paragraph with no links or anchors) and the full browser
and Rust batteries (the change is confined to the launcher, whose specs ran).

## Review gate

gpt-6.1-sol at high effort, as the plan asks: eleven rounds, every finding accepted and fixed, the last round clean. The
first round found five problems: a quick launch that could wait forever if the recent setups failed to load, a late
default that could rewrite an in-flight launch and its retry key, the selected session's folder losing to history after
a host round trip, a stale installation check left behind by a re-seed, and an order-dependent browser assertion. The
later rounds were narrower interactions with permission Undo and Clone's host binding, ending in a design where a
permission Undo restores the folder default as it stood when its warning appeared. After the third corrective round, the
scope reassessment the plan requires (a fresh review on Opus 5.5, the executing model) found no unnecessary mechanism.

### Landing

Landed as #2669, with the content the report describes and no changes. It was the second of two plans in this landing
round, after uninstall-guards-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained alias-in-add-host (adding a host that is already listed is refused, and the add host
dialog takes an alias), and uninstall-guards-oct22 (the helm's uninstall and setup safeguards) landed just before it in
this round. A fresh-context reviewer and I read this plan against both: they change how hosts are added, set up and
uninstalled, and this plan changes only which directory the session launcher offers for each host, so they do not
interact. Nobody had built the GUI with both this plan and the add host dialog's new alias field before; this round's
web-target lint, release web build and browser specs did.

#### Checks

This round landed two plans: the helm's uninstall and setup safeguards (uninstall-guards-oct22), and the session
launcher's per-host directory (per-host-last-folder). The first touches the same setup code as alias-in-add-host, which
landed on main just before this round, and neither executor's checks ran with alias-in-add-host on main. So this round
ran the whole Rust workspace, the launcher's browser specs and the provisioning browser spec on both plans stacked
together on this round's main. The first three items below ran before the landing's docs fixes to uninstall-guards-oct22
(three paragraphs on two docs pages); the website build ran again after them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `0ac84ed9-860d-47ec-ba61-0c1a1a4f516c`: 4325 passed, 40 skipped by design (helper entries that
  only run as child processes of other tests, and tests that need a real signed-in Claude or Codex). This includes the
  helm's setup and uninstall tests, and the tests that set up, update and uninstall Farhelm on this machine over ssh,
  which the uninstall-guards-oct22 report lists as skipped; they ran and passed here.
- `npx playwright test` in `e2e/` on Chromium and WebKit for the nine launcher spec files the per-host-last-folder
  executor ran (`yolo-guard`, `replace`, `terminal`, `destination-authority`, `quick-launch`, `sidebar`,
  `terminal-create-idempotency`, `clone`, `permission-switch`) and `provisioning`, three workers each with its own helm
  and supervisors, no retries, run `0c90ae2b-16a2-4053-95db-b34184859aed`: 779 passed, 1 failed. The failure was
  WebKit's "offer retirement releases only its owner: waiting behind another host" in `provisioning.spec.ts`: clicking a
  host menu's uninstall item kept finding it detached from the page until the test timed out. The test answers the
  uninstall requests itself, so the helm's uninstall planning these plans change never runs in it. Ten WebKit repeats on
  this stack (run `a50fb0eb-6a64-46bb-9850-427d9c965492`) failed once, and thirty on main without the plans, with main's
  own binaries and web build (run `0964225d-00ee-4deb-9daa-5c0227ae3bdf`), failed 7 times: six the same way, and one a
  step earlier, while the host menu was still opening. It is a failure main already has; it is recorded in FLAKES.md,
  with a Deflake entry in TODO.md, in draft PR #2673.
- Reused: none. Skipped: the rest of the browser suite (the launcher change is covered by its specs above, and the
  uninstall changes have no browser-side code), the doctests (no documentation example changed), the desktop smoke test,
  the native Mac app (not runnable here), the installer (unchanged) and the macOS-only installed uninstall suite (not
  runnable here; these changes are in the helm's uninstall from the hosts panel, not the installed uninstaller).

The report's checks ran the launcher's unit tests and the launcher's browser specs, some before its final rebase; this
round ran the whole Rust workspace and the same nine browser spec files on both engines on the plan as it landed.
Nothing in the executor's report became untrue at landing.
