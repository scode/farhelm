# The hyperlink specification omits an accepted cursor-tail exception

Reviewed commit: 5e155da11b72c25bde82c5b34daff9887cf233ee

## TLDR

The product specification promises to compare consecutive cursor-positioned rows of a hyperlink together, but the
accepted implementation sometimes compares the later row's tail alone. The landing report acknowledges the mismatch and
leaves it as a follow-up. The maintained product contract should describe the accepted exception.

## Details

Suggested bucket: `other`, for authoritative documentation correctness. Confidence: confirmed. `could_be_highest: false`
for this finding's established extra-warning/documentation consequence. `proposed_drop: null`.

At the reviewed planning commit, `plans/queue/grid-repeated-link-warning.md:157–161` accepts the cursor-moved
continuation exception and asks for a code-comment note. The specification step at `:149–151` only adds the positive
joining clause. Related assigned reports are `93b87949ed9ef0b51408f608a04ca947454a0be6` and
`beb9340ecc47028d101bdaa55e15686bb2b00484`; the landing report,
`plans/reports/grid-repeated-link-warning.report.md:102–107`, says only implementation notes record the exception.

At `beb9340ecc47028d101bdaa55e15686bb2b00484`, `SPEC.md:2319–2320` says: “Consecutive rows of the same hyperlink are
compared together, including links an agent continues after indentation with cursor movement.” `SPEC_impl.md:1719–1720`
instead excludes an inherited piece's original generated id when an unwrapped continuation still carries that old id.

The accepted implementation at `8b4f1e16496292ce18d556f0bc40a80ef42b4b8f` replaces a later piece's id with the first
piece's canonical id in `grid-buffer.js:328–331`. The recorded test, `grid-buffer.test.js:375–382`, splits
`https://apne` and `ws.com/x` across the relevant rows and expects only the second row from the hover-text comparison.
The false warning follows from that accepted fresh-comparison behavior, rather than a stale hover cache.

At inclusive review tip `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`, `SPEC.md:2352–2353` and `SPEC_impl.md:1772–1773`
still disagree and the same identity rule remains. Document the narrow accepted exception in `SPEC.md`, preserving the
agreed implementation.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

The landing report already names this follow-up. `g025-l2` is a different defect: repairing canonical dependency
invalidation refreshes the comparison but does not change its accepted cursor-tail result; editing this specification
does not fix stale-warning invalidation. The proposed spelling-classification exclusion under `g031-l1` is also
separate.

Caveats: this is not a request to restore broader joining or a claim of an undisclosed security regression. No
data-loss/security consequence or runtime reproduction is established. The separate potentially-highest
spelling-classification concern remains subject to its independent consultation.

Review provenance: automated committed-code trailing review, run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, reviewer
`g031`, original ID `g031-l3`; findings log lines 3, 6, 10 and latest line 14. The latest revision explicitly preserves
the separate `g025-l2` owner and consequence.
