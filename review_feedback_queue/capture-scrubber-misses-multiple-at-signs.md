# Capture scrubber misses local proxy userinfo containing another at sign

Reviewed commit: 8f2efb5fa5f4111f9855c13f63690e5ec83588fe

## TLDR

The capture scrubber and its immediate check both miss a local proxy URL with another raw at sign in its password. The
separate Rust check catches it when run; actual vendor display and publication of such a URL are unestablished.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The prior fixture-url-userinfo decision at TRIAGE_OUTCOMES.md:21690-21706 covers ordinary userinfo recognition; this
residual multi-at pattern survives its implementation. Independent consultation agrees OTHER under the
maintainer-tooling rule and retains the concrete scrub defect and conditional exposure. The --no-test route bypasses the
independent Rust validation; no fixture or credential was actually published.

## Recorded source assessment

Reviewed commit 8f2efb5fa5f4111f9855c13f63690e5ec83588fe. Synthesis rechecked replacement order, immediate refusal,
staging-to-repository copy and optional Rust validation. TLDR: if a captured screen contains a local proxy URL with a
raw @ in its password, the tool writes that URL and credentials unchanged to the fixture tree. Concrete input
http://user:p@ss@localhost:8080/backend-api/codex has no email-shaped match at :93; LOCAL_URL :100-102 admits at most
one @ before the host, so scrub :142-150 and check :152-163 both miss it, assuming the chosen credentials are not host
identity strings in forbidden. Recorder.save :276-287 writes both screen and title after that check; capture_harness
:731-734 copies them into the repository. --no-test is declared at :770 and main :802-803 skips independent validation
when it is set. Without that flag Rust screen_fixtures.rs:223,310 catches the local host after the copy, so this is not
a demonstrated bypass of Rust validation. Earlier record's parse_args/main references :768,807 were imprecise;
:770,802-803 are the exact references. Suggested bucket other under the explicit maintainer-tooling limitation: no
actual publication or catastrophic consequence established. Confidence confirmed for the scrubber contract on this
concrete text; likely for an end-to-end real capture, with the missing premise being whether the installed vendor
displays the configured URL with the extra @ intact. Available source checks cover the whole local call chain; vendor
implementation/display normalization is outside this source-only checkout and runtime/network checks are prohibited. A
retained vendor screen containing this URL would settle that premise. Proposed fix: bound the authority, identify the
host after its last @ as the Rust guard does, and cover multiple @ signs in replacement/refusal tests. proposed_drop:
false. possible_cover: none searched. Original withdrawal and reopening remain preserved; no consequence reduction in
this synthesis.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g036/c1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
