### What this was about

This batch fixes seven triaged bugs in the helm, CLI and Codex sidebar status. Closing or reloading a page could cut off
an accepted feedback send or leave helpers running while preparing a host update or removal. Stopping a host connection
immediately after starting it could leave it reconnecting without supervision. The triage decision was to fix these with
the existing helm-owned task mechanism and connection cleanup guard, with no new subsystem.

Three other bugs mishandled a chosen state directory: setup's sign-in command pointed at the default helm, token
rotation could change credentials before discovering that its recovery command could not represent the directory, and
literal `${...}` in an SSH socket directory was treated by SSH as an environment reference. The sidebar could also
request an answer because question-shaped text had been pasted into an unsent Codex draft.

### Things you should know

All seven outcomes are implemented; none needed the complexity fallback. Accepted feedback forwarding and update/removal
planning now finish independently of the requesting page, including their existing bounded cleanup. Removal still
requires the later explicit confirmation; closing a planning panel does not remove a host. An immediately stopped
connection now carries its cleanup guard even if supervision never got a chance to run.

Setup advice includes the resolved, shell-quoted directory when you chose one explicitly. Token rotation refuses a
directory that is not valid UTF-8 before making changes. Paths containing literal `${` use the existing SSH connection
mode without sharing a socket; ordinary dollar signs remain eligible for sharing. Codex queued questions are recognized
only above the last input prompt, so pasted draft text does not count, and a long draft does not hide a real question
above it.

The seven feedback files and their complete index entries are removed, and their execution records identify this draft
PR. A fixed changelog fragment covers the user-visible effects.

### Open questions and possible follow-ups

No decision is waiting on the maintainer. Two cancellation-specific runtime cases remain untested: feedback would need a
new outbound completion observer, and removal planning would need a gated probe fixture. The plan expressly allows
omitting a regression that needs new machinery while still shipping the fix. Existing functional tests and the generic
helm-owned-task cancellation test cover the surrounding behavior, and source review checked both callers; they do not
substitute for those two omitted direct cancellation observations.

### PRs

- [#1813](https://github.com/scode/farhelm/pull/1813/changes): all seven fixes, their regression coverage and triage
  bookkeeping, in one draft PR.

### Checks run, reused and skipped

Focused Rust execution used the recorder with pinned nextest and tmux, four global slots and no retries. The initial six
regressions passed 6/6 in `618dc90b-2e02-4ca6-b395-f599aa30c17b`. After correcting the connection-test ordering found by
review, that case passed 1/1 in `23bec8e8-5fa9-4e5e-a5e0-72899b0b4a9a`. Six deliberate controls restoring each tested
original production bug all failed the intended assertions in `b8c179bc-8fba-4ca5-af64-1b1e2fa75a92`; the controls and
their failed evidence are retained, and all six source restorations were verified.

The compatibility selection passed 158/158 in `a3451a66-5b24-4ee7-be27-a3db65728a89`: 60 CLI setup cases, 89 helm cases
and nine Codex screen-reader cases. Retained output contained no runtime `SKIPPED` refusal, and cleanup completed. After
the production dependency correction below, custom-directory sign-in advice passed again, 1/1 in
`eeb13412-4374-423d-95ac-b45e83934247`.

`cargo clippy --all-targets -- -D warnings` initially failed because the CLI's quoting library was declared only for
tests, so production setup advice could not compile. The existing library was promoted to a regular dependency without
changing its version or adding a mechanism. Both that command and `cargo clippy -p farhelm --bins -- -D warnings` then
passed. The first failed lint output is retained privately.

`cargo fmt --all -- --check`, dprint on changed Markdown, the isolated `python -B scripts/check-test-sleeps.py` and
`python3 releasing/check-changelog.py format` passed. The delay check inspected 282 delays with zero missing rationales;
the final fragment check covered 43 fragments. Formatting and fragment checks were repeated after inserting the actual
PR links.

The focused runtime evidence is reused across the final rebase from `499d6160` onto `20bb48cb`: upstream changes
concerned documentation, capture tooling and queue bookkeeping, with no changed Rust behavior. Earlier upstream
service-file readback changes were covered by the 60 setup cases. Moving the already-resolved quoting dependency into
production changed compilation availability, covered by both final lints and the final advice test, without changing its
runtime implementation. Later main changes only approved prior plans and removed their goal/report records; they do not
interact with this PR's reviewed base. No extra runtime run was justified by these changes.

No full Rust battery, browser suite, desktop runtime, installer or release gate was run. The fixes use existing request
ownership and cleanup, CLI advice, SSH arguments and pure synthetic Codex screens; focused runtime cases cover those
changed contracts. There is no changed browser flow, generated asset, desktop integration or release pipeline needing
those broader checks.

### Review gate outcome

A fresh native gpt-6.1-sol high reviewer inspected the exact diff against all seven ledger entries, the plan,
authoritative specifications and the complete test-authoring checklist. It found one ordering defect in the new
connection-cancellation test: opening the actor's gate before cancelled supervision finished allowed a valid publication
to race cleanup. The test now waits for supervision to finish while the gate remains closed, then releases it; a failed
release is allowed because cleanup may have dropped the receiver. The focused pass and old-guard mutation control
establish the intended boundary. No production-code or additional source findings remained.

The second fresh native gpt-6.1-sol medium wording reader found the commit and PR text accurate and
convention-compliant. Implementation stayed local under the required no-workhorse mode. Native actual model identity and
usage counters were not exposed. The executor did not mark the PR ready or merge it.
