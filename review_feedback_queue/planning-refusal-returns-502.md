# Planning refusals return 502 instead of 409

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Updating a host registered with a relative farhelm path fails with a gateway-style error that reads like a connection
problem instead of telling the user to register an absolute path.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F25 / COR-PLAN-REFUSAL-502`, tagged **possible**. Anchors and title: `provisioning/plan.rs:459-466`,
`provisioning/plan.rs:313-330`, `provisioning/service.rs:728-733`, `provisioning/http.rs:220-223` — Planning refusals
are returned as 502 Bad Gateway instead of the typed 409 refusal

Two planning refusals are configuration problems, not host failures: `plan_for_row` refuses an UPDATE whose recorded
`remote_farhelm` is relative (plan.rs:459-466), and `plan` refuses a destination path with no file name
(plan.rs:313-330). Both are built as `BackendFailure`, the type for failures reported by the host, and
`plan_update_unguarded` passes them straight up (service.rs:728-733). The HTTP layer's `provisioning_error`
(http.rs:220-223) maps every `BackendFailure` to 502 Bad Gateway. The comparable refusal from the reach check is sent as
`ProvisioningRequestError::Refused`, which maps to 409 Conflict, and http.rs states that the status must come from the
error type, not from its prose.

A refusal the user must fix in the host's configuration is thus reported as a transport or gateway failure, and a client
that treats 5xx as transient will retry instead of pointing the user at the fix. Suggested change: return these two as
`ProvisioningRequestError::Refused(reason)`.

User-visible consequence: updating a host registered with a relative farhelm path fails with a gateway-style error that
reads like a connection problem instead of telling the user to register an absolute path.
