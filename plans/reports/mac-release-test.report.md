### What this was about

The release test needs to catch a Mac app that will not start on an existing installation, or a new helm that cannot
update the remote supervisors an older release installed. This first slice supplies the parts that can be built on
Linux: a stable-candidate override for the in-app updater, plus an entry point and per-release procedure for an agent on
the Mac host to bring up disposable guests in Tart, the Mac VM tool. It follows the decision to use separate Mac and
Linux VMs, real Claude/Codex sessions locally, and plain commands remotely.

### Things you should know

The updater override selects a version only. It keeps the real download origin, stable-version grammar, newer-version
comparison, signature/trusted-comment verification and installer hashes. Invalid, empty, prerelease and non-text values
fail without falling back to the live latest release. Both automatic and requested checks share the captured value.
Restart removes it from the relaunch helper's environment.

The documents are an intended procedure, not a successful Mac test. They require fresh VM pairs for fresh-install and
previous-stable upgrade passes, retain real state, check restart/resume and local-host connectivity, update the remote
host and prove its original command kept running. They explicitly distinguish ordinary in-app updates, staged stable
candidates supported by the old app's override, and manual candidate installs for RCs or old apps without the override.
The manual path exercises the installer's version/checksum-file interface but skips the old updater's own probe and
verification; separate tooling uses the previous release's key ring so a new signing key cannot hide the old app's
refusal, and the report must show the remaining gap. The old app's automatic update must be disabled before its startup
check, so it cannot replace the previous-version fixture while state is being seeded. The remote command is observed
over SSH outside Farhelm, not through a redisplayed buffer. After an override-path restart, a requested check must name
the live `/latest` version in the hover when it differs from the candidate; unavailable relaunch logs are recorded as a
diagnostic gap rather than invented evidence.

### Open questions and possible follow-ups

No maintainer decision blocks this slice. Start the next agent on the Mac host with: “Read
`releasing/mac-vm-test/BRING-UP.md` and do what it says.” It must resolve the actual Tart images, networking, SSH,
computer use, environment forwarding, pre-start preference setup, signature tooling and log capture; write portable host
scripts; run both passes with the latest stable and its predecessor; correct the documents and open its own PR. Ubuntu
26.04 LTS, with 24.04 as fallback, is an unverified image recommendation. The prepared bases hold credentials and remain
private; all installs are inside disposable guests, never the host's live Farhelm.

The initial latest-stable dry run proves the ordinary update path only. Native override forwarding waits for a published
release carrying it; restart clearance waits for a real staged candidate upgrade. Those paths and manual RC installs
still need their own evidence. One previous-stable pair also does not establish the full support promise for every
stable release since v0.23.0. Staging/promotion workflow changes remain separate.

### PRs

- [#1682](https://github.com/scode/farhelm/pull/1682/changes): test-only stable-candidate selection in the desktop
  updater.
- [#1683](https://github.com/scode/farhelm/pull/1683/changes): Mac-host bring-up entry point and per-release recipe;
  removes the covered first-slice TODO.

### Checks run, reused and skipped

Ran `cargo check -p farhelm-ui --features desktop`,
`cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
`dprint check` on the changed Markdown, and `python -B scripts/check-test-sleeps.py` with the isolated pinned parser
interpreter (269 delays, none unexplained). The recorder wrapped `cargo nextest run -p farhelm-ui --features desktop`
with the desktop updater selection; all 34 selected tests passed: run `734d401d-9284-4c0c-a04f-de57b97cc6fa`, with four
nextest slots, zero retries and no tmux dependency. After the relaunch guard/test clarification, compile/Clippy and
source checks passed again, and the seven affected override/relaunch tests passed in run
`788c5483-ff28-414f-b8e4-9d19e2522fb0`. The earlier worker/install test results remain applicable because those
mechanisms were unchanged; the altered relaunch path was rerun explicitly.

The documentation PR ran dprint and a relative-link existence check; no runtime suite was warranted for those files.
Native Mac/Tart runs were unavailable on this Linux executor, and the documents mark that limitation. Browser tests were
skipped because no browser flow changed. No full workspace battery was needed for the version-probe seam.

### Review gate outcome

Both PRs passed independent Opus 5.5 high and GPT-6 Astra high reviews. PR 1 corrected the logging sentence, added an
environment-contract guard and desktop variable inventory, and clarified test inputs. PR 2 corrected evidence oracles,
old-release signing-key selection and the sequencing of native override verification. A fresh scope reassessment
confirmed that deferring unavailable native capabilities is the smallest coherent correction and needs no new release or
test infrastructure. Optional style suggestions that added no material clarity were left alone. Commit/PR wording was
cold-read independently; the final report receives a separate readability and public-hygiene cold read.
