# Linux ADD writes the frozen install before rechecking systemd availability

Reviewed commit: 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae

## TLDR

An approved Linux setup can install the binary and unit, then fail on reload, when the user service manager became
unavailable while the setup question was open.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:2788`.

SPEC.md:120–140 and 194–200 tie setup to an inspected usable user manager and a concrete approved plan.
SPEC_impl.md:7923–7926 requires revalidating facts the plan relied on. Planning tests for a usable user manager and
locates its unit directory (backend.rs:852–904,2737–2783,3810–3828). With no supervisor answering at confirmation,
service.rs:2707–2718,2785–2816 repeats that check only for a Mac plan. A Linux plan executes its frozen directory,
binary and unit actions before daemon reload (plan.rs:719–749,779–800. Installation.rs:64–111. Backend.rs:2918–2928).
Reload can therefore be the first failure after those writes, even though the same unavailable-manager result would have
precluded a plan. Revalidate Linux reach before executing, keeping the approved action list if the relevant facts still
agree. The companion stale unit-directory premise is narrower: source shows the old path is written and production
enables by unit name (backend.rs:2931–2949), but whether a particular XDG change moves systemd's effective lookup path
is unverified and is not needed for the confirmed ordering. Existing reach fixture asserts spaced-directory handling and
escaped-form refusal at planning (provisioning.rs:9285–9363), and the ADD confirmation fixture changes Mac reach only
(5180–5230). Bucket: other for partial setup and an avoidable retry. No established overwrite of an unrelated unit or
user-work loss. Confidence: source-supported for loss of manager availability before confirmation with no supervisor
answering. No runtime outcome observed. Possible coverage: none identified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p13`.
