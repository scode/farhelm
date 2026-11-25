# Update on an identity-unverified host fails with a misleading "identity changed / plan again" remedy

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

Updating a host whose supervisor answers but reports no install identity (while the entry has one on record) starts an
Update run that is guaranteed to fail. The failure says the identity "changed" and tells the user to "plan again" —
re-planning reproduces the same failure, and the real remedies (wait for the host to identify itself, or fix the
destination / remove the entry) are not named.

## Details

**other.** Source inspection; commissioner confirmed the planning guard. The run refuses safely before any mutation; the
trigger is the rare IdentityUnverified state; no security/data-loss route. FILTER.md rare-glitch/imprecise- diagnostics
considered: the misleading remedy fits the diagnostic branch, but the defect also includes the accept-then-fail flow and
a futile re-plan loop (more than a briefly-wrong display); the match being unclear, it does not match — recorded.

In `crates/farhelm-helm/src/provisioning/service.rs:1768-1779` (`plan_update_owned`'s Supervisor arm) the identity guard
is `if let (Some(recorded), Some(reported)) = … && recorded != reported`, so it fires only when both sides are Some. A
peer reporting None against a recorded Some passes planning; `expected_identity` stays Some(recorded).
`require_update_trusted` (service.rs:3195) refuses IdentityMismatch/Duplicate/OwnedElsewhere but not IdentityUnverified.
At execution, `revalidate`'s Update arm (service.rs:~3010-3019) refuses via
`expected_identity.is_some() && &host_identity != expected_identity` with "the supervisor identity changed after UPDATE
planning … plan again". None was reported at planning too, so nothing changed and "plan again" is futile. The codebase's
own wording for this state elsewhere names the right remedy (`attach_refusal`'s IdentityUnverified arm, service.rs:~380;
uninstall's `changed_identity`, uninstall_refusal.rs:153). The uninstall comment (service.rs:~2180) notes the planning
asymmetry is deliberate, so the defect is the misleading execution diagnostic and accept-then-fail pattern, not the
asymmetry.

Verify: drive a row with host_identity = Some whose supervisor answers hello with no identity, click Update, observe the
preflight failure and the re-plan loop. Fix shape: refuse at planning for recorded-Some/reported-None with the
IdentityUnverified remedy, or make the execution refusal distinguish "reported none" from "reported a different
identity" and name the wait/fix-destination remedy.

Reviewer metadata (area 5 chunk C, provisioning service first half): suggested bucket: other; confidence: confirmed
(deterministic code path; rare trigger the manager treats as reachable); proposed_drop: none; possible_cover: none
known. Commissioner validation: guard confirmed at the cited lines; coverage searched (queue — the IdentityUnverified
hits are Podman place-claim items, different scope; TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
