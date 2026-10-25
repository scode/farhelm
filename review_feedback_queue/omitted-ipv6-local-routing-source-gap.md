# Omitted IPv6 forms leave a conditional local-link permission gap

Reviewed commit: 609df3cdd656612c1aa70f9cc7debf067a8b2260

## TLDR

Some IPv6 forms receive public-link permission. A supported route reaching a local resource remains unestablished, so a
local authorization bypass is conditional.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `plans/reports/tunnel-port-and-link-ranges.report.md:49-50`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Reviewed report commit: 609df3cdd656612c1aa70f9cc7debf067a8b2260. TLDR: a Teredo or IPv4-translated local destination
could open using only a remote host's public-link grant if a supported network/browser route actually maps that literal
to a local resource. Pinned tip 463cac1528b5c5e259404975380d321a3fff0412 establishes the Farhelm authorization route:
browser.rs:58-82,95-115 omits Teredo 2001::/32 and translated ::ffff:0:0:0/96 from local classification;
approvals.rs:421-436 then selects links_without_asking and returns Raw immediately when the native opener exists;
browser.rs:140-166 passes the original URL unchanged to that opener after checking origin liveness; UI
desktop.rs:505-518 delegates it to webbrowser::open in the external system browser. Browser tests at browser.rs:228-308
cover only the enumerated ranges and do not refute these omissions. Missing SOURCE premise: a concrete supported
OS/browser routing or translation mechanism that reaches a local resource for one of these literal forms. No such route
was established by repository source; carrying IPv4 bits is insufficient. Do not replace that gap with an assumed
private translator or call it merely unmeasured frequency. Suggested bucket: conditional other; proven local
authorization bypass would be highest. Confidence: confirmed public classification/native opening path; possible local
reachability remains unverified. Investigation stops incomplete at that boundary under the source-only constraint.
Possible_cover: neither a041's exact listed-prefix extraction refutation nor this report's disclosure accepts or refutes
omitted-prefix product behavior. Proposed_drop: none; supersedes the initial report-only exclusion. Preserve for the
parent's fresh independent possibly-highest audit. No tests, network accesses or reproduction were attempted.

Provenance: `g138/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
