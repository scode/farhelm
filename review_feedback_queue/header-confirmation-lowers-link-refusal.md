# Header confirmation lowers link refusal beneath unrelated covers

Reviewed commit: b5c2b7bdf72d0a8ce17bd91e94d16d3de30efe33

## TLDR

While a session confirmation is open, a refusal explaining why a terminal link cannot open can sit beneath another
overlay. The link still refuses safely; its explanation may be concealed.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional layering mechanism**; exact rendered overlap and
frequency were not measured. No transfer, opening or security bypass is established.

`crates/farhelm-ui/assets/app.css:1437` lowers every disclosure-refusal message from layer 51 to 12 while a header
confirmation exists. This protects the header warning, but also lowers the explanation beneath unrelated covers such as
approval cards (`7649–7656`). `assets/approval-layout.js:19–43` places those cards over the terminal, and the refusal
occupies that same area (CSS `1401–1422`). `assets/terminal-files.js:406–411,507–517` still refuses the covered link
before transfer or opening.

The new regression explicitly permits approval overlap and checks only warning/refusal relative order
(`e2e/tests/terminal-link-clicks.spec.ts:1310–1324`). It does not establish that the refusal remains visible above other
covers. The lowering remains present at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`.

Possible cover: tip `TRIAGE_OUTCOMES.md:18856–18872` requires the warning above the refusal, but does not clearly accept
concealment beneath unrelated overlays. `SPEC_impl.md:2354–2370` still promises explanations above every link cover
except the trash flight. Prior redraw and tunnel decisions (`16440–16457`, `15895–15914`) address different triggers and
record completed repairs. The diagnostic filter needs a rare trigger; simultaneous overlays alone do not establish that.
No exact Planned, BUGS or existing queue coverage was found. Proposed drop: none; retain the acceptance ambiguity.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g025/f1`; independent auditor
`a005` found whole-finding exclusion ambiguous. This does not reopen the repaired concealed Delete warning.
