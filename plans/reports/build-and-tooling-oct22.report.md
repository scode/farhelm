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
