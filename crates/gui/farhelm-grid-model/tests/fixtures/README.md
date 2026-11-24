# Grid model test fixtures

NOTE: The protocol's golden corpus is not here. The tests read it in place from `crates/farhelm-proto/grid-fixtures/`,
so this model can never be checked against a stale copy of it.

## Recorded streams

`recorded/attach.jsonl` and `recorded/scroll.jsonl` are copied unchanged from
`crates/farhelm-ui/js-tests/fixtures/grid/` at reference commit `24bc058066fd57848e788b465719b0e6da2f98f7`, because that
directory goes away with the old GUI. They hold semantic JSON descriptions, not binary wire frames; the tests encode
each entry with the production encoder before applying it.

## Link oracle

`links-oracle.json` records the answers of the old web client's `grid-buffer.js` and `terminal-links.js` at the same
reference commit (their SHA-256 hashes are in the file and checked by the generator before it runs them). The generator
evaluates that shipping JavaScript directly; it needs no DOM and no terminal emulator. Regenerate it from the repository
root with:

```sh
deno run -A crates/gui/farhelm-grid-model/tools/generate-links.ts /absolute/path/to/a/checkout/of/24bc0580
```

The corpus covers destination comparison and presentation escaping, parsed authority display, OSC precedence and
generated identities, URL punctuation, label joins across hard and soft row breaks, repeated and ambiguous tags, and
join-budget exhaustion. Rust tests first decode grid cases through the real wire encoder and checker. The detector's
nonexistent negative evidence boundary is omitted because this model uses unsigned absolute line numbers. Invalidating
resets still retire the entire hover.

The prototype generated the first version of this oracle from Farhelm `0bc984b`. Regenerated from the reference commit,
which includes the later fix that keeps scrollback links on their displayed rows, every answer came out the same; only
the recorded source hashes changed.

The warning no longer follows the old client: SPEC.md's terminal links paragraph replaced its address-shape rule with
one that finds an address anywhere in a label. So the recorded warning answers are history, and they bind in one
direction only. Where the old client warned (`mismatch: true` in `policy`, a non-null `claimed` in `grids`) on a web
destination, the hover must still warn; where it warned on a complete label for a mismatch, the label the hover shows,
escaped, must equal `claimed`, which keeps row joining honest. Quiet answers are not asserted: the address tables in
`src/link_claims/tests.rs` and the multi-row cases in `links.rs` are the quiet side's specification. Everything else
(the cells of each link, its identity and evidence, the escaping and shortening, the parsed host and target) still binds
exactly, and the 414 policy cases with a non-web destination must stay outside the warning altogether.

`links-oracle-exceptions.json` is the record of every place the new rules make the hover depart from the safety check.
Each entry has a `kind` (`policy` or `grid`), a `key` (`{text, uri}` for a policy case, `{grid, line, result}` for a
grid answer) and the `rule`: the behavior, in words, that explains the departure. The tests refuse an unknown kind, a
duplicate key, and an entry no case uses, so the list can only shrink when the code is fixed and must grow by hand, one
explained case at a time.

The JavaScript shortening policy counts UTF-16 units. A slice that splits a surrogate pair is stored with the
replacement character a native text widget renders; the omitted count still follows the original units. This corpus
checks pure recognition and disclosure policy. Pointer ownership, geometry admission and what covers a link belong to
the GUI's own tests.
