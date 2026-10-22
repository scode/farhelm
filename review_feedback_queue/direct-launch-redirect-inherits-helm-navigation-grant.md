# Direct-launch redirect inherits the helm navigation grant

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

An untrusted address opened directly in the browser may redirect into the standalone helm. With a saved credential and
successful startup, the new page could open a session and displace another viewer. Whether the documented direct-launch
permission deliberately includes these redirects is unsettled, and the complete browser/application sequence is
unverified.

## Details

`crates/farhelm-helm/src/middleware.rs:184–224` admits a request without Origin when its Fetch Metadata classification
is `none`. The
[Fetch Metadata working draft](https://www.w3.org/TR/2026/WD-fetch-metadata-20260921/#sec-fetch-site-header) skips
redirect-chain relationship checks for direct user-initiated navigation classified that way. The inspected
[Chromium helper](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/services/network/sec_header_helpers.cc)
(blob `8d51fb761a89cfd6a1752eef48156831ad133e33`, inspected 2026-10-22) returns the no-initiator classification before
examining a redirect target. These sources support the conditional metadata mechanism, not observed browser behavior.

Farhelm can automatically choose a session on startup (`farhelm-ui/src/list/view.rs:2924–3008`). Its first terminal sync
is treated as opening (`assets/terminal.js:2968–2988`), uses ordinary attachment (`3129–3150,3985–3990`), and can
displace another lease (`farhelm-helm/src/terminal.rs:334–341`, `client.rs:3548–3557`;
`farhelm-supervisor/src/service/handlers.rs:1511–1629`). That chain still requires a valid saved credential, successful
UI/bootstrap and asset loading, an eligible session, and an incumbent viewer. No combination of those conditions was
executed or observed here.

This is separate from `cached-directory-ui-skips-navigation-guard.md`: the redirect reaches the network guard and needs
neither cached HTML nor directory serving. The cache fixture's results are not evidence for this redirect. The initial
direct user navigation is essential; an ordinary hostile page link is not the claimed trigger. The redirecting party
also needs the running helm's address. Native-opened or tunnel-delivered links have not been shown to acquire this
classification.

Suggested bucket: **provisional highest** only if this exceeds the intended browser trust boundary. Confidence: **likely
conditional issue, with confirmed predicate and source-level attachment semantics**. Actual supported-browser redirect
construction, final Origin absence, other browser restrictions, authenticated startup and viewer displacement remain
unverified. No credential disclosure, attacker terminal input, code execution, process loss or lost work is established.
Source and primary-document inspection only; no reproduction is requested.

Possible cover: `SPEC.md:2926–2928` preserves address-bar/bookmark launches. The implementation follows its literal
header predicate; the missing decision is whether that grant includes redirects through an untrusted origin.
`SPEC.md:3277–3284` requires practical browser attacks, and `TRIAGE_OUTCOMES.md:3156–3168` discarded an earlier
hypothetical/excluded-actor navigation concern. Neither clearly settles this particular trigger and consequence.
Independent coverage review did not approve a drop. No exact Planned, BUGS, queue or filter match was established.
Proposed drop: none; triage should settle the direct-launch grant's scope before choosing a remedy.

Provenance: persistent reviewer `A01-origin/A01-origin-3`, independent `origin-redirect-static`, parent source and
primary documentation verification. This remains an unresolved conditional finding, not a verified full-product security
defect.
