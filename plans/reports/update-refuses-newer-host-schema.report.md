# Refuse updates that cannot read a host’s database

## What this was about

Trying a development build can upgrade a remote host’s saved data. Updating that host back from the installed release
then went ahead because development builds rank below releases, even when the release could not open that database. The
replacement supervisor restarted repeatedly and the host stayed unreachable until the newer build was put back.

The maintainer chose to keep devbuild’s deployment behavior and make Update refuse before changing the host when its
database needs a newer build. Execution originally blocked because the reviewed fix exceeded the agreed size limit. The
maintainer accepted the built fix, including hosts using a different protocol version, and waived that limit.

## Things you should know

Update now compares the database version reported by the host with the version its replacement supervisor can open. It
checks both when preparing the update and again before carrying it out. The refusal explains the mismatch and says to
update from a build at least as new as the one that last ran there. The check also works when the host and helm use
different protocol versions.

Protection requires a helm with this fix and a supervisor that reports its database version. Older helms ignore the new
information; an older supervisor that does not report it keeps the previous behavior. A development build made before
this fix can therefore still leave a host unprotected. The existing warning about development builds sharing remote
hosts still applies.

This does not change devbuild’s choice of supervisor, migrate any database, or change normal build-version ordering. Two
branches assigning different meanings to the same database version remain outside this plan. The devbuild instructions
now carry the update-back caveat.

## Open questions and possible follow-ups

No maintainer decision remains. The originally accepted fix was 288 changed lines of code, tests and documentation.
Final review corrections bring that to 329, using the same design: clearer failure text and compatibility caveats,
constructor documentation, and stronger tests. The older-peer limitation and the same-number migration collision remain
as described above.

## The PRs

- [#2649](https://github.com/scode/farhelm/pull/2649/changes): refuse Update before installation when the host reports a
  database newer than the replacement can read.

The PR remains a draft for the landing monitor. It has not been merged by this executor.

## Checks run, reused and skipped

Final recorder run `0623921f-3c78-4a1a-b8e2-782a7320651b` passed four selected cases, with 2,749 filtered out, four
nextest slots and zero retries. Its complete source record is the dirty tree based on `6b0d76cf` with the final review
corrections; the later ledger and message edits do not change tested behavior. Pinned nextest and tmux identities
matched, output and the test report were complete, and no selected case reported a runtime skip. The child command was:

```sh
cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(=provisioning::tests::update_refuses_a_host_whose_database_schema_is_newer) | test(=provisioning::tests::update_refuses_a_host_running_a_newer_farhelm) | test(=io::tests::handshake_refuses_protocol_mismatch) | test(=tests::new_hello_json_decodes_under_a_legacy_pre_host_identity_decoder)'
```

This covers higher-schema refusal and equal/missing-schema acceptance, revalidation at confirmation on both protocol
paths, the existing build-version refusal, schema retention through a protocol mismatch, and legacy decoding with the
literal serialized key pinned.

The real supervisor greeting test passed in recorder run `6a011d31-6d7e-4233-83c6-0f54ccf92bd3`: one executed case
checks the reported schema through the production connection path across a supervisor restart. Its complete recorded
source was based on `6b0d76cf`. Later changes only improve refusal text, expand the refusal test, pin a serialized key,
and correct documentation; they leave that producer behavior unchanged, so this result is reused.

The pre-correction focused run `204dfa7c-b226-4487-8607-6c995f234de1` passed four cases. An additional selector named a
helper rather than a runnable test and supplied no producer coverage; the separate producer run above corrected that
gap. The final corrected run supersedes those four results for current refusal and wire behavior. Prior run
`2b780c27-fd76-4892-8055-2fdcd143f36e` passed 432 selected protocol, provisioning and connection tests on a complete
recorded dirty tree based on `ba942de5`, with pinned tmux matched. That is supporting evidence from before the rebase,
rather than a fresh whole-suite verdict.

Final `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, touched-Markdown `dprint check`,
changelog format validation, and the isolated test-delay checker passed. The instruction checker passed with zero errors
and one existing warning about an unrelated triage citation. The delay checker inspected 314 delays with no missing
rationale.

The whole Rust battery, browser tests, installer and real-host deployment were skipped. The changed behavior is in host
Update admission and the supervisor’s greeting; focused transport and provisioning tests cover those boundaries, and
compilation checks the combined fixtures. The tests use isolated supervisors and injected provisioning operations. No
live installation was updated or otherwise changed.

## Review gate outcome

The required single Opus 5.5 reviewer at high effort passed the final change. Its initial review found misleading
refusal attribution, an incomplete compatibility caveat and test gaps. Those were corrected, and the same reviewer
verified all six findings were resolved with no new issues. A separate scope reassessment at the executor’s own model
found no unnecessary complexity.

The native resume audit and the complete upstream-change audit found no conflicting decision or design change. The only
rebase conflict was in the feedback index; both plans’ removals were preserved. The ledger now records the maintainer’s
size waiver and completion. Commit and PR wording passed a fresh cold read.

### Landing

Landed as #2649, as built, per your 2026-10-23 answer. It was the first of three plans in this landing round, before
dialog-saves-oct22 and build-and-tooling-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained launch-refusal-visibility (the launcher's refusal alert and folder warning), and
earlier today alias-in-add-host and uninstall-guards-oct22 changed the same setup code in the helm. A fresh-context
reviewer and I read this plan against them and against the other two plans in the round:

- The new database version on the connection is optional and left out when there is none, and the message it travels in
  accepts unknown fields, so older and newer helms and supervisors still talk to each other. No released version speaks
  the protocol it lands in yet, which fits the rule for additive protocol changes.
- Every fake host and test that alias-in-add-host and uninstall-guards-oct22 added carries or ignores the new field.
- The new refusal runs after the existing "host runs a newer Farhelm" refusal, when Update is planned and again when it
  is confirmed, whether or not the host speaks the helm's protocol. Uninstall ignores it, since it installs nothing.

#### Possible follow-ups

- Found at landing: SPEC_impl.md does not yet describe the database version the supervisor now reports, and its sentence
  that a development helm "never refuses Update as a downgrade" could mention this refusal beside it.

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

The report's checks were selected protocol, setup and connection tests; this round ran the whole Rust workspace, the
provisioning browser spec and the CentOS provisioning test with all three plans together. Nothing in the executor's
report became untrue at landing.
