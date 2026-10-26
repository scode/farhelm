# Repeated SSH begin markers can collapse an intervening span

Reviewed commit: 973fbf693cddac36ab51b7ecb6659e17758e43bc

## TLDR

The provisioning test's begin/begin/end parser removes the intervening span. A supported unrelated stanza layout and
stale per-run alias selection producing actual config loss remain unestablished; balanced writers do not prove that
route.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `scripts/test-provision-centos.sh:170-175`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis of g276/f3 at reviewed commit 973fbf693cddac36ab51b7ecb6659e17758e43bc. Verdict remains AMBIGUOUS;
retain conditional OTHER parser concern, not a confirmed production user-work-loss finding. TLDR: if identical openings
enclose unrelated SSH stanzas before one closing marker, the Boolean scrubber deletes them and the caller can publish
the result (170-175,198-201,355-378). Newly checked history does not supply that missing input. Exclusive base
5c6e49937b1e779be2a5445059637aadf3757356 already has random aliases, atomic guarded balanced insertion
(81-95,148-158,387-405). Original fixed-marker writer 88631754ebc14d70b09d3ed13c58788a385c3f30 uses fixed markers
(69-74), scrubs before adding (227-234), builds a balanced prefix followed by the original file (257-271), and copies
through the destination (272). This establishes an old partial-write risk but does not establish duplicated openings
enclosing unrelated stanzas. Source changes 75966c4a66a15030c4e9fc4c5d2063fb824081f3,
8c7a6067ee72b4ca8d6882f93ef7199f748b8252 and dea00cbba321e628f1a285857712afc949b98d46 show the per-run marker migration,
atomic guarded publication and global-scope restoration; none supplies a concrete incidental writer of the claimed
malformed layout. Missing potentially-highest route is SOURCE evidence of accidental duplicate openings with unowned
stanzas between them, plus selection for a stale per-alias sweep. Runtime frequency is a separate unmeasured issue.
Exact ownership acceptance at pinned tip SPEC_impl.md:5033-5046 permits recognizing own files and processes by naming
convention, but does not explicitly assign every byte between repeated marker lines in a shared user file to tooling.
Source recognizes exact marker-delimited blocks; that does not by itself settle ownership of the ambiguous malformed
span. possible_cover remains incomplete: naming acceptance and SPEC.md:3255-3278 deliberate setup; no deliberate edit or
supported incidental merge writer has been shown. Missing-end ledger repair 18454-18471 remains separate and
source-refuted under f1. proposed_drop: no blanket coverage/refutation; retain conditional OTHER with missing
potentially-highest route. No new defect, no reproduction, no extra discovery search. Investigation stopped incomplete
at the source/ownership premise as instructed.

Provenance: `a062/f2`, `g276/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
