# Large trash listings lose the global partial-size warning

Reviewed commit: 4f200ea902679492f69eadd13cb963db52a77077

## TLDR

With more than 10,000 archived entries on one host, the trash headline can present an incomplete measured size as a
complete total.

## Details

Suggested bucket: `other`. Confidence: confirmed statically; no large archive collection constructed. Source: separate
adversarial trailing reviewer. This is an adjacent pre-existing gap in the host-summary correction. Proposed drop: none.

`crates/farhelm-ui/src/list/trash.rs:148–149` excludes over-limit hosts from deletion targets. Global `total_bytes`
still sums their known sizes (`561–579`), but `total_partial` examines unknown sizes and unchecked entries only through
those targets (`580–592`). A settled successful listing with 10,001 IDs and an unknown measurement is omitted from the
uncertainty calculation; `609` displays an unqualified total. This remains at the review tip.

Verify with a synthetic successful listing below the reply-size ceiling but above the deletion-ID limit, including both
known and unknown sizes. Global presentation must derive uncertainty from listed evidence, independently of which hosts
are eligible for deletion.

Possible cover: SPEC_impl.md, “Archived-checkout trash,” accepts refusing over-limit deletion batches, not reporting
unknown sizes as known. `trash-listing-exceeds-control-frame.md` concerns an unsuccessful oversized reply; this requires
a successful listing. The completed host-summary ledger outcome excludes global changes. No matching acceptance, Planned
item or BUGS entry was found.
