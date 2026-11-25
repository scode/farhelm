## What this was about

November 24 triage covered how Farhelm checks container hosts, builds recipes and cleans up containers and images. It
also covered the container launcher, recipe deletion and gaps in their tests. The plan accounts for all 27 outcomes in
12 draft PRs.

## Things you should know

- Container cleanup used to read the full result of Podman inspection. A container with enough metadata could exceed
  Farhelm's read limit and be left behind. Cleanup now reads the small set of fields it needs. A private rootless Podman
  run confirmed the inspect output, and fixtures covered cleanup of retained and missing containers.
- A fresh Podman check now uses the saved installation and its matching host connection. A verified host can take an
  unused claim from an entry refused as a duplicate, including when that refusal names a third host. Adoption clears the
  prior check and releases its unused claim. A settled Podman claim refusal releases an unused claim on the former
  machine or account. A pending launch, container, image or helper protects its claim until cleanup.
- New container refuses an invalid checkout name before starting preparation. A refused attempt keeps the repository and
  name available to edit. The host field keeps its ordinary height, and the launcher shows that deleting the last
  session removes the container and its files.
- Four conditional outcomes stayed unchanged under their size limits. Two fixture gaps would need setup that genuinely
  crosses create-time collision or pre-status transport failure. Adding the missing tunnel marker beside a field needs a
  placement and a test for row removal. Removing the brief Connecting state after Suspend and making the follow-up check
  quiet require changes to both the status and retry policy.
- The checkout-name preflight uses a validator and an existing test that also serve Tensorlake, so both provider paths
  were checked. The helm triage added a case where the SSH client exits during Setup. Its assertion and this plan's
  proof that the installed helper ran are both in the combined test. The helm and hooks/trust triage added screenshots
  under separate keys, all preserved.
- After validation, source inspection through main revision `08238da47` covered the rest of the hooks and trust plan. An
  invalid checkout name is refused before the approved trust setting is applied during preparation. Restoring unanswered
  hook offers when returning to a launcher tab is independent of the Container host layout. The drafts stay on the base
  validated at stack tip `901eb85d`.
- The Environments plan covers a checkout preview that fails after an existing host wakes and the permanent-removal
  notice for a new checkout on an existing environment. This plan refuses invalid names before preparing a new container
  and explains the last-session lifetime of that container. The global checkout history plan shares some launcher
  fixtures and changes where suggestions come from. The validation here still precedes preparation for an invalid name.

## Open questions and possible follow-ups

These three follow-ups are recorded in TODO.md.

- **Keep host proxy settings out of Podman recipe images.** The triage decision was to keep host proxy settings out of
  images. Prepared images can still inherit settings from older software-image layers, and an existing prepared image
  can be reused without rebuilding. The follow-up covers both cache transitions and checks the actual boot file inside a
  fresh container.
- **Verify container hosts before trusting results or sending private input.** Triage asked Farhelm to send private
  input only to the selected machine and account and reject command results from another destination. The command runner
  currently collects output after exit and delivers private input before it has endpoint proof. A check also gathers
  facts across several commands. The follow-up covers verification before accepting those results or sending private
  input, preservation of the right capabilities for existing resources after a refused check, and cleanup after the
  original destination returns. The claim fixes here address specific registry transitions while that wider work is
  pending.
- **Show container recipe admission failures before recording build ownership.** A recipe build can pass its first host
  check, then fail to start its build or preparation helper when the connection is resolved again. The follow-up covers
  showing the refusal before build ownership is recorded, including cancelling cleanly and clearing a reservation when
  the helper was never started.

## PRs

- Fixes cleanup failing on a full, oversized Podman inspection.
  [#4880](https://github.com/scode/farhelm/pull/4880/changes)
- Defers proxy settings retained in image layers, stale settings after removal, and the missing boot-file check to the
  proxy-image follow-up. [#4883](https://github.com/scode/farhelm/pull/4883/changes)
- Defers place verification in leftover cleanup, setup scripts and creation proxy reads, and the existing command proof
  to the verification follow-up. [#4884](https://github.com/scode/farhelm/pull/4884/changes)
- Fixes the saved Tailscale SSH mode check in container claims and recipe writes.
  [#4885](https://github.com/scode/farhelm/pull/4885/changes)
- Fixes Delete for a recipe whose failed rebuild left no resources on a removed host. It defers early Rebuild admission
  to the helper-admission follow-up. [#4888](https://github.com/scode/farhelm/pull/4888/changes)
- Adds later build and preparation helper admission to that same follow-up.
  [#4889](https://github.com/scode/farhelm/pull/4889/changes)
- Adds tests for the first Delete confirmation, the warning and buttons fitting at minimum window size, and keeping the
  captured image until the last real reference is gone. [#4894](https://github.com/scode/farhelm/pull/4894/changes)
- Fixes late refusal of invalid checkout names, the stretched Container host field, and the hidden last-session lifetime
  notice. [#4907](https://github.com/scode/farhelm/pull/4907/changes)
- Makes the absent-proxy test prove that the installed helper ran. Records no change for the create-time collision and
  pre-status diagnostic fixture gaps. [#4909](https://github.com/scode/farhelm/pull/4909/changes)
- Fixes transfer from a host refused as a duplicate of a third entry and prevents a late refresh from settling against
  earlier saved settings. The broader stored-capability verification goes to the named follow-up.
  [#4917](https://github.com/scode/farhelm/pull/4917/changes)
- Fixes stale checks and unused claims after Adopt and unused prior claims after a final Podman duplicate refusal.
  Defers the full verification decision for those outcomes and the wrong-machine runtime cache to the named follow-up.
  [#4921](https://github.com/scode/farhelm/pull/4921/changes)
- Bounds the unanswered container host diagnostic. Records no change for per-field tunnel marks and Suspend's transient
  Connecting state and retry ladder. [#4927](https://github.com/scode/farhelm/pull/4927/changes)

## Checks run, reused and skipped

At rebased tip `901eb85d`:

- `cargo nextest run --locked -p farhelm-helm --lib` with focused filters passed all 29 cases with no runtime skips in
  recorder run `f5309e56-dc9c-43c3-b656-d505a624abb8`. The selection covered Podman place checks, adoption and refresh,
  the combined Setup probe fixture, transport and Tailscale diagnostics, early checkout refusal and cold container
  launch.
- `python3 scripts/check-gui-goldens.py` with the New container, managed-checkout refusal, unavailable session and Hosts
  details scenes matched all eight light and dark references in `304b9ebc-489c-4a53-9122-2f564b81eb1d`.
- `cargo fmt` in check mode, `dprint check`, `python3 releasing/check-changelog.py format`,
  `scripts/plans-queue.py check` and `git diff --check` passed. `python -B scripts/check-test-sleeps.py` inspected 309
  delay calls with zero unexplained.

The rebase preserves the reviewed source, test, specification and image-reference patches. The earlier runs still cover
those changes. The revisions below are recorder base revisions with the respective plan edits present in the working
tree. The recorder IDs identify the captured source and test selection.

- A local Deno test driver confirmed the fields returned by the production inspection command against real rootless
  Podman in a private store from `2b547557` in `de754341-e87d-4de1-8a61-b6b4b4b6d2b7`. Filtered `cargo nextest run`
  cases at the same base exercised cleanup and ownership behavior in `c6a447c4-0ce9-4c47-bc86-412287840a45` and
  `e15904a0-30d3-4d55-94d8-a01fca2bf189`.
- Filtered `cargo nextest run` cases covered host place and Tailscale guards from `b6c2ed07` in
  `e224e0af-5b97-41d6-beb4-53425d81607b`, removal after a failed rebuild from `697e2b60` in
  `aa9dcd22-563c-43d1-971d-a3e2533a816e`, and recipe confirmation and cleanup from `7325bc38` in
  `212a2620-ed06-45ed-aca0-d7680375e9c5`.
- Filtered `cargo nextest run` cases covered the checkout name validator, HTTP refusal and provider launch from
  `c4fd7e0e` in `f79ff286-bb2c-472a-b720-d1bfa839f6a3`, `555f92cb-30cd-4e00-bc26-eabb5bc055b9` and
  `00df0f53-ee4f-41fe-9a73-9d8d21c33c4b`. A full `check-gui-goldens.py --update` at the same base produced four reviewed
  pictures in `e2066bc0-19e5-4581-ae3e-92d9623f16cf`. The selected recapture `5bbdd1ab-9d5e-4696-9f0c-063b98c379bd`
  matched their bytes.
- Filtered `cargo nextest run` cases covered claim refresh and transfer from `257776c2` in
  `a4510b02-d05d-4354-a092-f21db4fbdb46`, and adoption, claim refusal and pending-reference retention from `65957550` in
  `f75c6477-de99-45f9-a6d4-5c023d1dff94`. Selected `check-gui-goldens.py --update` captures of the existing launcher and
  settings views matched their published references at those respective bases in `0012726f-e3ba-4ce7-823c-7f89af9c821f`
  and `281a54af-2251-466b-83c9-893d9d9e39db`.
- From `adde519f`, filtered `cargo nextest run` cases covered the real command-capture fixture and shared diagnostic
  formatter in `60090a17-c768-413f-86b2-fb6fead9580c`. The full `check-gui-goldens.py --update` run
  `e7c48cf7-f02e-4853-813e-946256bb67f7` produced the two reviewed unavailable-session pictures. The ordinary Hosts
  details picture stayed equivalent in both appearances.
- The full runtime battery was skipped because the focused checks cover the changed paths. Deferred behavior has no
  production patch to validate.

## Review gate

Each draft passed an independent source review. The independent visual reviews cleared the affected launcher, recipe
deletion and container place views in both appearances. The maintainer's standing override ended cold reviews of commit
and PR wording during this plan. The source and visual reviews continued under the plan's requirements.
