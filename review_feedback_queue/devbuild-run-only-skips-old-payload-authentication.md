# Run-only skips authentication of an older staged payload

Reviewed commit: be29e118a353b20b8e40331b6abc0a815fb1bcdf

## TLDR

Launching an earlier staged development build bypasses the new tmux signature check. Whether the repair must cover
operator-staged older builds is unresolved; the claim that verification runs every time is too broad.

## Details

Suggested bucket: **high**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`be29e118a353b20b8e40331b6abc0a815fb1bcdf:TRIAGE_OUTCOMES.md:18770-18782; scripts/devbuild-mac.sh:13-17,48-50,261-267,305-327`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Independent targeted reconciliation of g213/f2. Reviewed commit be29e118a353b20b8e40331b6abc0a815fb1bcdf. Existing
narrow AGREE is unchanged: build modes verify authenticated sums, hash cached/fresh tmux and then stage the checked
executable. New remainder: --run-only intentionally launches the earlier staged app and supplies its existing payload
directory (:305-327), so it can reuse a pre-fix stage made by
d8e96d449e9eefc586d9a964a698137dc42a90a5:devbuild-mac.sh:589-607 using unsigned sums. This mechanism is
source-established; absence of a measured historical compromise is not source refutation. The literal every-run claims
at reviewed script:48-50 and ledger:18777 overstate what runs. Established consequence is inaccurate maintainer-tooling
documentation/completion, suggested other bucket, confidence confirmed. For a security/incomplete-repair claim about
retained pre-fix stages, exclusion verdict AMBIGUOUS: script:13-17,261-267 expressly supports running an earlier build
even after the checkout changes, and pinned SPEC_impl.md:4866-4886 makes explicitly selected local payload directories
operator-trusted on any build; however, the reviewed ledger:18770-18774 requires verification before trusting tmux and
rechecking/refetching cached tmux without explicitly settling retroactive retained-stage treatment. The local-directory
rule answers the helm's behavior, not by itself the script's obligations for bytes it previously downloaded. No pinned
exact decision requiring or exempting retroactive authentication/invalidation was found. This is an unresolved
repair-contract premise, not a runtime-frequency caveat. Retain g213/f2's old-stage remainder conditionally rather than
claiming full source refutation or confirmed mandatory retrofit. Suggested high only conditionally for the substantive
maintainer-tooling trust consequence; no unproved catastrophic remote harm or highest consequence is established under
the tooling rule. possible_cover: script's explicit earlier-build contract plus SPEC_impl operator-staged authority,
partial/ambiguous as described. proposed_drop: yes for repaired fresh/cache staging mechanism; no approved drop for
old-stage contract remainder; retain confirmed wording mismatch as other. This does not reopen the shared verifier or
make a blanket run-only/local-file-safety exclusion. f1 umask and f2 terminfo verdicts remain limited and unchanged. No
tests, execution, network access or reproduction.

Provenance: `a061/f3`, `g213/f2`, `g276/f4`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
