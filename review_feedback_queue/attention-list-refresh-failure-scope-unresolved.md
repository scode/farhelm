# Attention list refresh failure scope unresolved

Reviewed commit: 5ea72c7e7d9a72318da3fabf16a304f2ed2772c8

## TLDR

A connected host's session list can stop refreshing without entering the attention list. Whether that omission violates
the intended scope remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `plans/queue/host-attention-list.md:11-16,43-49,82-87 at 5ea72c7e7d9a72318da3fabf16a304f2ed2772c8`.

Source assessment (read with the later corrections below) (review record g008/g008-9, event 19):

Reviewed planning commit: 5ea72c7e7d9a72318da3fabf16a304f2ed2772c8. TLDR: a host can stay connected and current while
its session list repeatedly fails to refresh; the proposed always-visible attention list does not name that failure
condition, so a user may still have to open Hosts/details to discover why that host's sessions stopped updating. The
omission and reachable failure state are source-backed; whether omission contradicts the intended scope is unverified
after investigation. Assigned record evidence: original plan:11-16 quotes the goal of showing any host currently having
a problem so nothing needing attention is hidden; original:43-49 enumerates Problem conditions without
Connected.refresh; original:82-87 excludes a connected current host. The explicit decisions original:118-139 do not
discuss refresh failures. Necessary source: UI lib.rs:1088-1104,1204-1227 keeps RefreshHealth::Failed independent of
Connected; hosts.rs:1133-1153 displays 'the last session refresh failed, so its sessions are last-known' in details
while phase_display_label:384-404 remains Connected for a current build. Helm manager.rs:4770-4796 returns Failed with
end_connection None for a refused/error listing and retains the previous cache; :4573-4584 publishes Connected with that
health, so this is not merely an unproduced enum state. Store errors can do the same at :4917-4929.
SPEC_impl.md:5300-5316 requires keeping the previous cache on failed refresh. Timeout/identity-change failures are
distinct and do end the connection (SPEC_impl:5279-5299), so those do not establish this omission. Missing premise:
whether the maintainer's broad 'any host ... has a problem' includes a failed session-cache refresh or whether the
plan's enumerated phase/provisioning/tunnel set deliberately narrows it. Available source establishes the failure and
omission but cannot establish that product decision. The explicit connected-current exclusion is a plausible acceptance
basis, not proof that the maintainer considered failed refreshes; authoritative clarification of this case would settle
it. If included, classify Connected with Failed refresh as a Problem with Details; if excluded, qualify the broad
promise so later readers do not infer comprehensive host-health coverage. Suggested bucket: other, for
attention/diagnostic discoverability only; no established new data/process/security loss. Confidence: unverified after
investigation (source omission/state confirmed, intended-scope violation unresolved). proposed_drop: false;
possible_cover: the plan's explicit connected-current exclusion and pinned SPEC.md:2175-2177 are possible acceptance, no
queue/plan/filter coverage search. Caveats: source-only, no runtime reproduction; this is a finding about the assigned
plan's claim/scope, not an independent full audit of the later classifier or an instruction to drop another group's
implementation finding. At the pinned tip the already-read SPEC.md:2175-2177 still excludes usable current connections
without another named warning; no intervening implementation defect or repair is claimed. This is independent of known
g079:l2 dismissal membership and g090:c5 geometry. The lead is resolved into this qualified finding.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g008/g008-9`; supporting
records `g080/i7`, `g081/f1`, `g079/i8`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
