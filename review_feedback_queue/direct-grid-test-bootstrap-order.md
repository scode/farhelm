# Grid integration test races an unrelated asset load

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A healthy grid terminal can fail its integration test when the unrelated xterm asset loads slowly, because the test
asserts the asset is loaded before establishing that premise.

## Details

Source coordinates from the original records: `e2e/tests/grid-terminal.spec.ts:75-78`. References below use these paths'
basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

This supplements the preceding l4 finding without withdrawing any of its source chain or uncertainty about runtime
frequency. The complete Dioxus CLI v0.7.10 packages/cli/src/build/web.rs:263-278 confirms write_js_glue_shim is called
before HTML generation. :283-313 is the full non-awaited __wbg_init(...).then(...) shim; :380-412 selects default HTML
absent a crate index.html and injects resources/bootstrap; :485-503 emits static script tags only for
options.static_head(); :517-534 emits the async module bootstrap. Farhelm at introduction lib.rs:1324 uses plain
asset!("/assets/vendor/xterm.js") without options. Local manganis-core-0.7.10/src/js.rs:28-45 initializes JsAssetOptions
with preload:false and static_head:false. Thus the xterm asset is not quietly inserted as a parser-owned script by stock
asset defaults, and the load-event gap described in the preceding finding remains applicable. Source search also
confirms repository references pin dx 0.7.10 (Dioxus.toml:1-28 and scripts/devbuild-mac.sh:390). No test, browser,
generated-artifact execution or live endpoint experiment was used; remote reads fetched only pinned public dependency
source.

The original assertion is e2e/host-terminals/grid-terminal.spec.ts:83-86 at 2668f28; tip is
e2e/tests/grid-terminal.spec.ts:75-78. Preserve the conclusion that e16's move did not introduce this assertion. It is
the initial introduction within the full pinned range that matters. Fix direction: explicitly await Terminal's
script-ready witness before testing loaded:true/built:0; keep grid readiness independent. Suggested bucket: other,
maintainer test false failure only. Confidence: confirmed permitted scheduling route from source; not runtime reproduced
or frequency measured. proposed_drop: false; possible_cover: none. Could be highest: no. Interface g014/g017: bootstrap
and asset-default premises are established; challenge those specific source premises if another pinned build step
changes them, rather than treating page.goto(load) alone as an xterm-ready witness.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g021/l4`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
