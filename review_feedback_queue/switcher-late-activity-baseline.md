# Switcher quick launch captures activity after its close handoff

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other, unverified after investigation. The
accepted pick at `list/view.rs:5250–5267` still crosses DOM removal at `5277` before quick launch at `5321,5335`
captures its baseline at `5033`. `SPEC.md:1364–1368` and `SPEC_impl.md:1187–1190` retain the dispatch-baseline contract.
No exact current coverage or later fix was found. Browser refutation and the missing supported native input ordering
remain; source relocation does not establish that physical event sequence.

## TLDR

A quick launch picked from the switcher captures its input baseline after closing the switcher. On native desktop this
might absorb later typing and permit unwanted selection or naming, but the required native event ordering remains
unverified after investigation. The ordinary browser investigation did not establish that harmful input window.

## Details

Bucket: **other**, from the established capture/lifecycle gap. Original suggested high remains conditional on proving
the unwanted native navigation route. Confidence: **unverified after investigation for the practical defect**;
post-handoff capture itself is confirmed by source. No runtime trace or reproduction was performed.

Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`. At the reviewed tip,
`crates/farhelm-ui/src/list/view.rs:4924` accepts the switcher pick without capturing activity, releases isolation at
4945, waits for DOM removal at 4951, and calls `quick_launch` at 4995/5009. That function snapshots the baseline at
4707. `SPEC.md:1340-1343` counts activity after quick dispatch; `SPEC_impl.md:1180-1184` requires capture before fresh
reads or host wake. The required DOM-removal handoff at 1605-1609 does not expressly waive that activity boundary.

The reviewer inspected matching Dioxus 0.7.10 and js-sys 0.3.106. Web rendering/flush is synchronous
(`dioxus-web/lib.rs:178-265`), eval awaits a Promise (`document.rs:207-265`), and executor work uses microtasks
(`js-sys/futures/queue.rs:30-79`). DOM-removal observation and continuation drain before another physical-event task; an
await alone does not establish a browser input gap. The original generic-browser impact is unsupported.

Native eval results return through IPC and a later native event (`desktop/query.rs:44-121`, `webview.rs:308-317`,
`app.rs:309-318`); task polling and DOM acknowledgments are separate (`webview.rs:537-589`). Physical input uses the
synchronous custom-protocol path (`interpreter-js/ts/native.ts:397-409,564-590`, `desktop/webview.rs:43-88,190-194`).
These paths do not establish a concrete supported ordering in which a later counted press reaches Rust before the
baseline snapshot. Further source checks found closing-state guards against another switcher close/reopen and no
opener-refocus on the accepted quick launch. The activity listener counts independently of focus, so those guards do not
themselves settle the missing ordering.

What remains missing is a supported native sequence placing a counted physical press/relay between accepted pick and
post-handoff snapshot. A native event trace spanning pick, removal/query IPC, input and snapshot, or a platform ordering
guarantee, would establish or refute it. Available source did neither; no runtime experiment was authorized in this
review. Capturing at the accepted pick is the likely repair if that route is established.

No exact queue, current ledger, unlanded plan, Planned TODO, accepted bug or whole-finding filter cover was found. The
cached-switcher-label item concerns a pending opening read, not this accepted-pick boundary.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `switcher-pick-baseline`; `g022/l1`. Original
confidence, conditional high consequence, renderer checks and revisions remain in `prep/groups/g022/findings.jsonl`.
