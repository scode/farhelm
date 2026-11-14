# Empty proxy variables disappear after the container user manager restarts

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: an explicitly empty proxy variable is installed into a new container's live user-manager
environment but disappears after that manager/container restarts. Farhelm therefore does not preserve the exact selected
creation-time proxy environment. Source: PodmanPlaces probe retains ManagerValue::Value("") in ReadyPlace.proxy
(environment/podman_probe.rs:616-626); proxy_file at podman.rs:1457-1469 emits NAME="". proxies writes that file to
environment.d at 588-617, separately unsets/imports names into the live manager at 621-637, and admitted create retries
deliberately leave the creation snapshot alone (1225-1230). Direct scripts export explicit empty values through
build.rs:420-429. The empty-versus-absent invariant is stated in tests.rs:542-575, whose formatter assertion expects
HTTP_PROXY="" but does not exercise the real generator. Concrete supported-distribution evidence checked without
execution: Ubuntu 24.04's official package page identifies systemd 255.4; its official source archive
https://archive.ubuntu.com/ubuntu/pool/main/s/systemd/systemd_255.4.orig.tar.gz contains basic/env-file.c with
value=NULL at 28-29, no allocation for empty quotes at 98-126 and 202-214, and merge_env_file_push discarding !value at
532-535. The complete official distribution patch archive's diff headers,
https://archive.ubuntu.com/ubuntu/pool/main/s/systemd/systemd_255.4-1ubuntu8.18.debian.tar.xz , do not alter env-file.c,
env-util.c or environment-d-generator.c (one env-util.h patch was present; it does not replace the parser functions).
The official source's merge_env_file at 557-569 uses t

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g065/l1`.

### g065/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: an explicitly empty proxy variable is installed into a
new container's live user-manager environment but disappears after that manager/container restarts. Farhelm therefore
does not preserve the exact selected creation-time proxy environment. Source: PodmanPlaces probe retains
ManagerValue::Value("") in ReadyPlace.proxy (environment/podman_probe.rs:616-626); proxy_file at podman.rs:1457-1469
emits NAME="". proxies writes that file to environment.d at 588-617, separately unsets/imports names into the live
manager at 621-637, and admitted create retries deliberately leave the creation snapshot alone (1225-1230). Direct
scripts export explicit empty values through build.rs:420-429. The empty-versus-absent invariant is stated in
tests.rs:542-575, whose formatter assertion expects HTTP_PROXY="" but does not exercise the real generator. Concrete
supported-distribution evidence checked without execution: Ubuntu 24.04's official package page identifies systemd
255.4; its official source archive https://archive.ubuntu.com/ubuntu/pool/main/s/systemd/systemd_255.4.orig.tar.gz
contains basic/env-file.c with value=NULL at 28-29, no allocation for empty quotes at 98-126 and 202-214, and
merge_env_file_push discarding !value at 532-535. The complete official distribution patch archive's diff headers,
https://archive.ubuntu.com/ubuntu/pool/main/s/systemd/systemd_255.4-1ubuntu8.18.debian.tar.xz , do not alter env-file.c,
env-util.c or environment-d-generator.c (one env-util.h patch was present; it does not replace the parser functions).
The official source's merge_env_file at 557-569 uses this parser. Tagged upstream v255 and v257 carry the same
empty-value callback; v233 fileio.c:710-750 also did. Ubuntu/Debian custom image bases are expressly supported by
SPEC.md:472-475, with no extra container systemd version floor stated. Thus a supported concrete Ubuntu distribution
route exists; no claim that every supported value was refuted remains. The generator discards the assignment, rather
than emitting an empty value, on the next user-manager startup. This establishes exact-environment loss; it does not
establish a specific network-client fallback or security consequence.

Original potential highest consequence preserved: if dollar-sign transformation of credential-bearing proxy text
selected an unintended endpoint, credentials could be exposed. The dollar-specific encoding premise is source-refuted:
systemd v233 NEWS:324-329 introduces environment.d; its environment-d-generator.c:61-77 calls merge_env_file; v233
env-util.c:518-537 collapses $$ to a literal dollar, and v255:665-685/v257:718-738 retain that branch. The earlier
upstream-main-only rationale was insufficient and has been replaced with historical parser and actual distribution
source evidence. The original hypothetical credential route was never established. Keep that history for the parent's
independent drop/priority judgment; it is not a final drop decision.

Contract: SPEC.md Container sessions says newly created containers inherit selected place user-manager proxies and
retain the snapshot across later changes; SPEC_impl.md Disposable environments / Podman places, creation and images
requires the selected values in environment.d for future user-manager/session inheritance. proxy_file's empty form
violates that exact persistence contract. A fix needs a generator-compatible way to represent an explicitly empty
variable, rather than assuming empty quotes survive parsing. Suggested bucket: other for the established empty-to-absent
correctness consequence, only a provisional recommendation; original conditional high and possible-highest endpoint
consequence are retained above. could-be-highest: original potential security route considered, but not established for
this confirmed empty-value defect. Confidence: confirmed source/contract analysis, including actual Ubuntu package
source; not behavior observed during execution. proposed_drop: false for the empty-value finding; source refutation of
the original dollar mechanism is proposed to the parent only. possible_cover: none. All readily checkable parser/version
premises for the concrete supported Ubuntu route resolved. No tests, builds or reproductions ran.

Final synthesis check: the actual Ubuntu 255.4 source archive environment-d-generator.c:44-79 was read as well, closing
the generator-caller premise: load_and_print builds env only through merge_env_file at 59-65 and prints only entries in
that array at 67-78. No separate empty-assignment recovery exists there. Re-read SPEC_impl.md Podman places, creation
and images (6537-6572): it requires selected variables in environment.d for deterministic user-manager inheritance. The
unrelated supported-setup proxy exception at SPEC.md:5219-5221 concerns SSH ProxyCommand aliasing, not proxy environment
values. No supported-value exclusion or missing readily checkable premise remains for the established Ubuntu empty-value
route. The original dollar-sign hypothesis remains separately source-refuted, with its potential highest consequence
retained above for independent parent judgment. No runtime evidence added.

Discovery-turn interface synthesis, before new search: supplied g003/f1 establishes a DISTINCT creation-time cache
snapshot defect. PodmanPlaces::fresh accepts a same-route success up to 300 seconds old, while create obtains
ready.proxy through age-unbounded runtime; proxies only reads the CONTAINER manager for XDG_CONFIG_HOME and
writes/imports the passed values. The host-manager change therefore need not reach a new environment, which then
preserves that snapshot after admission. This does not refute or duplicate l1: l1 holds even when a genuinely current
explicit-empty value was sampled, because its environment.d encoding is lost on later user-manager startup. Conversely
fixing the empty serialization does not refresh the place snapshot; fixing cache sampling does not make
NAME=empty-quotes survive the parser. g003 retains its own source-confirmed network/configuration consequence and other
recommendation with no established credential route, while l1 retains its original potential highest history and
empty-value consequence. No duplicate finding is claimed or drop/priority accepted. Supplied g003/f3 initial-launch
cleanup acceptance is also independent of this proxy persistence contract. No new source premise or execution
observation added by this reconciliation.

Returned proposal (claims-041, event 15): **retain-verified**, suggested bucket **other**.

An explicit empty proxy value is imported into the current container user manager but serialized as empty quotes that a
concrete supported systemd parser discards. The assignment is then absent from generated persistent settings after
restart. Dollar escaping is a separate, source-refuted mechanism whose original possible security consequence must
remain in the parent's drop register.

Original stronger consequence: Initial dollar-sign transformation hypothesis conditionally suggested high and possible
highest if credential-bearing proxy text selected an unintended endpoint and exposed credentials. The reviewer first
withdrew it based only on upstream-main documentation, reopened the supported-parser premise, then source-refuted the
dollar mechanism using historical parsers while retaining a distinct empty-value defect. All of that history remains
relevant to the independent drop/priority check.

Unresolved premise or limitation: No missing parser/encoder premise for the checked Ubuntu255.4 route. The package-page
version assertion timed out, but actual official255.4 package source/patches were checked. Specific network-client
fallback, material launch failure, unintended endpoint selection, credential disclosure or loss of work remain
unestablished.

Proposed basis: Retain empty-to-absent persistence as other for the established exact-environment consequence. Send the
dollar-specific refutation and any lowering of the original security hypothesis to the independent drop check; this
consultation does not approve that drop.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-empty-proxy-persistence-with-separate-dollar-refutation-and-priority-gate. Full041
exact empty Value survives input/live-manager import but encoder writes NAME=emptyquotes; actual supported Ubuntu255.4
env-file parser allocates no value for empty quotes and generator ignores NULL, so persistent setting is absent after
restart. Source/patches checked; package web page timed out, not credited. Retain source-confirmed exact-environment
persistence defect, proposed other absent a concrete network/credential/work route. Separate doubled-dollar hypothesis
refuted with actual historical v233/v255 parsers, not merely upstream-main docs; original higher/security endpoint
scenario and withdrawal/reopening history remain for fresh gate. Do not refute all proxy/network defects or
stale-probe064c4, nor call a restart a repair of this wrong persistent record.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g065 record](records/review-through-00de0b6c/groups/g065.json). Record ID: `l1`.
