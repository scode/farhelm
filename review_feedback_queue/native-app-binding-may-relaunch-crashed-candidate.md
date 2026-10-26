# Native app binding has an unresolved relaunch premise

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979

## TLDR

The test provider's app-binding operation might relaunch a candidate that crashed after the PID check. The exact
provider-version semantics are unavailable, so a hidden false pass remains a conditional source lead.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `releasing/mac-vm-test/guest_ui.py:114-123,148-161`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit b1eed46975aab16ccc2217b799d5d9a045d8e979. Conditional OTHER, not confirmed. TLDR: if the native
provider's path-based getApp launches a missing app, a candidate that starts and then crashes between process
observation and native binding can be relaunched by the harness and appear to have restarted successfully. The
source-established portion is separate process observation and later binding: guest_ui.py:114-123,148-161;
guest_operations.py:558-561 forwards the generated getApp after the readiness receipt. managed_result at
guest_tester.py:287-294 checks a later candidate version but not continuity with the observed desktop PID.
validate_result:504-507 requires the owned restart event; this does not eliminate a replacement after that event's
process check. The disjoint-PID readiness check distinguishes old from new at its observation, not provider behavior
afterward. Geometry validation is confined to window-drag and bonus native-preflight (guest_tester.py:301-307,508-525),
not update-relaunch. UNVERIFIED SOURCE PREMISE: the exact guest provider's getApp launch-on-missing behavior. Static
repository searches found only calls, warnings and mocked tests; test_guest_ui.py:80-100 uses canned provider replies
and cannot establish real provider semantics. The earlier log's statement that tests/docstrings 'assume' launch
capability is not evidence that the provider implements it. guest_operations.py:579-580 resolves the provider from the
configured Mac desktop bundle, which is not present on this Linux machine. Local plugin-cache filename/content searches
found no implementation or version-matched getApp documentation. No provider/native API was invoked, no network/SSH or
reproduction was attempted. Runtime occurrence is additionally unmeasured. possible_cover: none identified in previously
searched pinned SPEC.md, SPEC_impl.md, TODO Planned, BUGS.md, queue INDEX, TRIAGE_OUTCOMES.md and plans/queue; the
related release-tool ledger mechanisms differ. proposed_drop: none. No established security/user-work impact, not
possibly highest. Synthesis stops INCOMPLETE on this precise source-premise gap; not ready for separate discovery. To
settle, inspect the exact guest bundle's static getApp implementation or authoritative version-matched documentation,
without executing it.

Provenance: `g312/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
