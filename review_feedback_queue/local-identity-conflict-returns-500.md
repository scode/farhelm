# A local identity conflict during discovery returns 500 instead of 409

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After reinstalling the supervisor on the helm's own machine, re-probing it reports a server error instead of the
expected "identity changed, adopt or fix" conflict — hiding the recovery path and alarming monitoring for a routine
state.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p2).
Coordinator confirmed the untyped error, the fallthrough mapping, and the typed SSH equivalents.

A host reporting a different identity than recorded is a user-actionable fleet state (adopt-or-fix). For SSH hosts,
registration returns typed `HostStoreError::IdentityMismatch`/`IdentityClaimed`, mapped to 409 with structured
recorded/reported fields (lib.rs:1733-1742, 1808). The local row instead does
`bail!("the local
supervisor identity could not be registered: {outcome:?}")` (service.rs:480-482) — untyped,
`Debug`-rendered. `provisioning_error` (http.rs:216-229) recognizes only `BackendFailure` and
`ProvisioningRequestError`, so this falls through to `http_error`, which maps unclassified errors to 500 (lib.rs:1757,
pinned by test). Reachable by reinstalling the local supervisor and re-probing.

Suggested fix: translate a non-`Recorded` outcome into the store's typed errors
(`IdentityMismatch`/`IdentityClaimed`-family) instead of `bail!`, so the existing 409 mapping applies, with
recorded/reported identities as fields, not `Debug` text.
