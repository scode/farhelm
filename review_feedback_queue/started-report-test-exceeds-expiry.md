# Started-report test waits across the report's real expiry

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

The browser test can fail after a correct launch because its brief Started report expires while the test waits for an
unrelated successful preferences response. No actual occurrence has been measured.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source timing defect**. This is not an
observed latent flake. Reviewed change: `112fd9d812e95df7f69f66c85820d76faaf89582`; relevant source is unchanged at the
reviewed tip.

`e2e/tests/nonblocking-launch.spec.ts:1005-1011` observes the Started report, polls `readPreferences`, switches from
Hosts to Sessions and requires that report visible again. The test installs no controlled page clock. The product sets a
generation-owned report and clears it after `sleep_ms(4_000)` (`crates/farhelm-ui/src/list/view.rs:5676-5686`).
SPEC.md:1345-1350 expressly makes it a display-only report lasting about four seconds; panel navigation does not extend
it. `helpers/fleet.ts:1092-1095` awaits a real API GET and JSON body without tying completion to that remaining
lifetime.

A response arriving after the report expires can still be successful and well within the test's budget. The final
visibility assertion then fails permanently although the launch and expiry behaved correctly. Control the page clock for
report-lifetime assertions, or move unrelated API checks outside that real-time window.

No exact acceptance, Planned item, unlanded repair, BUGS, queue, current-ledger or filter cover was established.
Expired-lock and no-mount-window records involve different premises. Do not add a FLAKES record from source suspicion
alone. No delay injection, browser test or network request ran.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g024/l10`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): nonblocking-launch.spec.ts:1005-1011 is
unchanged: observe Started, await the real preference read, switch to Sessions, then require Started again.
view.rs:6007-6016 still clears that report after 4000 ms under its identity, consistent with SPEC.md:1374-1375's
about-four-second lifetime. No test latency bound was added. The false-failure timing route remains source-confirmed;
incidence was not measured.
