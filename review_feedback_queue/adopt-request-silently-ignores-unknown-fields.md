# Adopt request silently ignores unknown fields

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A typo'd or misspelled extra field in an adopt request is silently dropped with no error — tooling sending something
unrecognized gets no signal on an identity-trust decision.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

`AdoptReq` (`crates/farhelm-helm/src/hosts.rs:674-679`) has no `#[serde(deny_unknown_fields)]`, while its siblings
`HostSpec` (hosts.rs:381-390, with documented rationale: "a typo that is silently ignored produces a registry row that
dials the wrong thing ... and reports success") and `AliasSpec` (hosts.rs:404) both carry it. So
`POST /api/hosts/{id}/adopt` with `{"reported": "x", "force": true}` (or any typo'd extra key) decodes fine and drops
the extra field. A fully-misspelled `reported` still fails loudly via the missing-field error, but any ADDITIONAL key a
client believes is meaningful — a flag, a future field — is silently discarded on an identity-trust decision: operator
tooling gets no signal it sent something unrecognized. No behavioral impact today (the server acts on `reported` alone
with safe defaults), hence low; the module's own stated rationale for the siblings applies verbatim and the fix is one
attribute. Possible, low.

Suggested fix: add `#[serde(deny_unknown_fields)]` to `AdoptReq`, matching `HostSpec` and `AliasSpec`.
