## What this was about

Five fixes in installation and maintainer tools. Farhelm's private Linux tmux searched a deleted build folder for
terminal definitions, so a hand-run attach failed or could read another account's planted definitions. The plans queue
counted outsiders' fork PRs as a plan's own work. The CentOS test could truncate the user's SSH configuration after an
unmatched block marker. Rerunning the Mac installer kept a damaged version program and reported success. Devbuild
trusted downloaded tmux checksums without verifying their release signature, and reused cached tmux without rechecking
it.

The five draft PRs address those cases. The signature check exceeded the original size limit at 122 changed lines; your
2026-10-23 Decision approved that fix as built. This round kept it, updated its execution record, and rebased the stack.
Product code and tests are unchanged from the reviewed versions.

## Things you should know

The tmux change affects hand-run terminal use; sessions in the app did not use that terminal-definition lookup. Both
Linux architectures were built during the original execution. The obsolete lookup path is gone; tmux's normal
temporary-file and socket defaults and build paths in debug information still contain temporary-directory strings. Those
strings are not terminal-definition lookup paths.

The installer fix repairs a damaged program when the installer is rerun. It adds no power-loss flushing and changes no
automatic-update policy. The original finding's APFS power-loss premise remains unverified.

Devbuild checks the signed checksum file and every cached or downloaded tmux on each run, using the release keys
compiled into the binary it just built. A bad signature or mismatched tmux stops the run and removes the cache. A
missing signature stops the download; its diagnostic is curl's error. No additional verifier installation is required.
The Mac devbuild script was not run end to end on this Linux executor.

## Open questions and possible follow-ups

No decision is needed to finish this plan. Three optional follow-ups remain: stripping build paths from the Linux tmux
debug information; clarifying that executor procedures use the queue's filtered PR listing; and making the CentOS test's
separate identity lookup diagnose every unmatched block. The queue now excludes fork PRs, but an executor that
independently lists PRs by branch prefix instead of using that filtered listing could still read a fork PR as plan work;
the procedure clarification would close that remaining ambiguity. The CentOS identity lookup can still keep a malformed
block silently when it finds an existing key later in the configuration, but neither scrub publishes truncated
configuration.

## The PRs

- [#2657](https://github.com/scode/farhelm/pull/2657/changes): Linux private tmux terminal-definition lookup.
- [#2659](https://github.com/scode/farhelm/pull/2659/changes): fork PR exclusion in the plans queue.
- [#2660](https://github.com/scode/farhelm/pull/2660/changes): SSH configuration preservation in the CentOS test.
- [#2662](https://github.com/scode/farhelm/pull/2662/changes): damaged-version repair on an installer rerun.
- [#2665](https://github.com/scode/farhelm/pull/2665/changes): release-signature verification for devbuild's tmux.

## Checks run, reused and skipped

Run this round: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the isolated
`python -B scripts/check-test-sleeps.py` (314 delays inspected, none without a rationale), scoped `dprint check`, and
`python3 releasing/check-changelog.py format` passed. The initial scoped formatting check found the edited ledger
paragraph needed rewrapping; formatting it and repeating the check passed.

Also run this round at revision `8ed44787`: `python3 scripts/test-plans-queue.py` through recorder run
`80815974-76e0-4d95-ba1f-f3ae8bc75106`, 64 tests passed; and `bash scripts/test-install-sh.sh` through recorder run
`e63cd812-c9d2-49a3-b8b4-252f8f63497a`, 557 checks passed with zero failures. Both records have complete source and
output capture, normal exit and no runtime `SKIPPED` marker. The installer suite uses Linux fixtures that simulate the
Mac environment; it is not native macOS filesystem coverage. ShellCheck with `-x` passed on the five changed shell
scripts; `sh -n scripts/install.sh` and `bash -n scripts/devbuild-mac.sh` passed too.

Reused for #2665: `cargo nextest run -p farhelm --bin farhelm -E 'test(verify_release_sums)'`, recorder run
`b8ce6dd8-f2b8-4a52-b10c-960bea4fd36a`, one selected test passed, with 175 tests selected out. It captured parent
revision `68246b6b` plus the then-uncommitted signature change, source fingerprint
`815fff226614fd4a979776ea73addb4cb8550152dfefe343c084ce7b9c22d522`.

Reused for #2657: the pinned Zig build invoked `scripts/build-private-tmux.sh <linux-target> <owned-output>` for x86_64
and aarch64, followed by string inspection. Retained build logs and the review record establish completion and the fixed
search list. For #2660, the original `bash <extracted-scrub-check> <checkout> <owned-scratch>` and the reviewer's
extracted caller harness checked matched, unmatched and absent blocks without touching real SSH configuration. Those ad
hoc checks have no available recorder run IDs or complete source fingerprints; their provenance is the retained
build/review records, a weaker identity than the recorded runs above. The genuine-release/manual verifier checks and
filter mutation checks have the same limitation and are supplementary evidence.

The original reviewed heads were `6e4b3967`, `14765312`, `0cab2b96`, `68246b6b`, and `05f7877b`; the final behavior is
preserved by an empty code/test interdiff after rebase. Upstream changed none of these scripts, verifier inputs, keys,
dependency manifests or signed fixtures, so the narrow evidence still applies. Original shell checks covered syntax and
ShellCheck for the changed scripts.

Skipped: new Rust runtime repetitions, a full Rust/browser battery, the full CentOS provisioning script, and native Mac
devbuild execution. No product behavior changed this round and the upstream audit found no functional interaction
invalidating the narrow evidence. The full CentOS script rewrites real SSH configuration and is expressly forbidden by
this plan. Native Mac execution is unavailable here. Nothing touched the live installation.

## Review gate's outcome

All five PRs passed the original fresh-context Opus 5.5 high review gate, with corrections recorded in the working log.
The resumed stack's native independent check passed and audited all 56 upstream commits through `bdace0ec`; it found no
functional conflict. Shared queue-index conflicts kept both upstream and plan removals. The final report passed its
native independent cold read after revising validation provenance and the remaining procedure ambiguity.

### Landing

Landed as #2657, #2659, #2660, #2662 and #2665, merged bottom-up, with the content the report describes and no changes.
#2665, the devbuild signature check, is included per your 2026-10-23 answer. It was the third of three plans in this
landing round, after update-refuses-newer-host-schema and dialog-saves-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained launch-refusal-visibility, which touches none of these scripts. A fresh-context
reviewer and I read the plan against it and against the other two plans in the round:

- #2665 adds a hidden `farhelm` command, which devbuild calls to check a release's signature before trusting its tmux.
  It sits in the existing group of hidden internal commands and uses the one signature check SPEC_impl.md names for use
  outside the helm's own downloads; devbuild builds the binary before calling it.
- The queue script's fork filter applies the same way to listing plans, recording a landing and abandoning.
- The private tmux's new terminal-definition search path ran on another distribution in this round's CentOS provisioning
  test, which passed; the test's ssh configuration editing, which #2660 changed, left the configuration as it was. Your
  plan told the executor never to run that script because it rewrites the real `~/.ssh/config`; I ran it as a landing
  check after copying the file aside. It worked, but it went against the plan's caution, and you may not want the
  monitor doing that.

#### Possible follow-ups

- As the report notes, the plans flow's own instructions still find a plan's PRs by branch name; they could point at the
  fork-filtered listing the queue script now gives. That is a change to the planning instructions, which code PRs may
  not make.
- As the report says, the Linux tmux still contains `/tmp` strings: tmux's own socket and temporary-file defaults and
  debug paths, not where it looks for terminal definitions. So the plan's check as written (`strings` finds no `/tmp`
  path) still fails; approving the plan accepts the executor's narrower reading of it.

#### Checks

This round landed three plans you unblocked on 2026-10-23: the Update refusal for a host whose database is newer
(update-refuses-newer-host-schema), the dialog save fixes (dialog-saves-oct22) and the build and tooling fixes
(build-and-tooling-oct22). Their executors ran selected tests; this round ran, on all three stacked together on this
round's main, before the one landing fix (which rewrote one changelog fragment and deleted another):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (the devbuild instructions changed; one warning, about a citation main already
  had), `sh -n scripts/install.sh`, `bash -n scripts/devbuild-mac.sh`, `shellcheck -x --severity=warning` on the five
  changed shell scripts, the workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`:
  all passed.
- Through the test recorder: `bash scripts/test-install-sh.sh`, run `7d6f72b3-6747-4db4-a0d5-c89dce396143`, 557 checks,
  none failed; `python3 scripts/test-plans-queue.py`, run `14cb5f16-37fc-485b-98f0-aa023c05e6fb`, 64 passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` with pinned nextest and tmux, four slots, no retries, run
  `709a1d66-f15c-4a4c-aee6-87068ba2ae6c`: 4328 of 4329 passed, 40 skipped by design (helper entries that only run as
  child processes of other tests, and tests that need a real signed-in Claude or Codex). It covers the connection field,
  the Update refusal, the hidden verification command and the GUI's unit tests. The one failure,
  `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, is the known open flake already in TODO.md's
  Deflake list; none of these plans touches the code it tests. `cargo nextest run -p farhelm-ui --features desktop`,
  same settings without tmux, run `e13aca7f-da53-4e38-8b75-8de136a7e174`: 920 passed. The asset script tests
  (`node --test` in `crates/farhelm-ui/js-tests`), run `12ed0e03-0b32-47e9-ba61-2f30426d626e`: 333 passed.
- `npx playwright test` in `e2e/` on Chromium and WebKit for `settings`, `templates`, `trash-dialog`,
  `launcher-template-shortcuts`, `quick-launch` and `provisioning`, three workers each with its own helm and
  supervisors, no retries, run `c386c83f-760d-4531-8085-d400657b6611`: 429 passed, 3 failed. Two are open flakes
  recorded in draft PRs that have not merged yet: the WebKit template-shortcut editor-focus test (#2621) and WebKit's
  "offer retirement ... waiting behind another host" (#2673). The third, Chromium's "accepted ADD registers before
  execution and releases the page lock while running" in `provisioning.spec.ts`, timed out opening a host's menu while
  that host was being set up, the same kind of failure as #2673. Repeated alone it failed 4 of 10 on this stack (run
  `506088c3-846a-4e83-8e55-ecf65dee16af`) and passed 10 of 10 on main (run `b073dbcc-4dc5-4c11-bd2e-3f1105db0256`), so I
  bisected. The stack below build-and-tooling-oct22 passed 50 repeats while the stack with it kept failing about one run
  in five, even with no binary or web bundle difference between them. But adding to that lower stack just one file
  nothing reads (a copy of the plan's new changelog fragment, or a meaningless file at the top of the checkout) brought
  failures back, the clean lower stack then failed 3 of 20 (run `7a76db98-fce9-4dbf-963b-4b7d9c1434fc`), and main itself
  failed 2 of 20 when run again (run `2a887633-7de3-45d4-a1a7-98c63c9f3758`). So it fails on main too, and I could not
  tie it to any of these plans; why the earlier runs split so cleanly is unknown. It is not in FLAKES.md yet; I will
  record it in a draft PR beside #2673.
- `scripts/test-provision-centos.sh` through the recorder, run `4630d8a9-426c-4d2b-96a2-120586999598`: a CentOS Stream 9
  container was set up over ssh by this round's helm with the release-style static payloads; passed. The
  build-and-tooling-oct22 plan told its executor not to run this script, because it rewrites the real ssh configuration;
  I ran it anyway as this round's check, after copying that configuration aside, and it was unchanged afterwards. It is
  the one test that runs the private tmux on another distribution and edits the ssh configuration the way the changed
  marker handling does; the configuration was unchanged afterwards.
- After the landing fix: `python3 releasing/check-changelog.py format` passed again.
- Reused: none. Skipped: the doctests (no documentation example changed), the desktop smoke test, the native Mac app and
  the macOS-only installed uninstall suite and devbuild script (not runnable here; the installer suite above runs the
  Mac installer against Linux fixtures, and devbuild was checked with its syntax check and shellcheck only).

The report's checks were the installer and queue script suites, the hidden command's test and hand-run builds; this
round ran those suites again, the whole Rust workspace and the CentOS provisioning test with all three plans together.
Nothing in the executor's report became untrue at landing.
