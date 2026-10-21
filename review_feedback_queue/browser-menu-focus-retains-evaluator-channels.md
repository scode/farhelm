# Browser menu focus retains evaluator channels

Reviewed commit: 1c671d26b1a5afdfc7a12d0fb986f349578a5250

## TLDR

Browser menu opening and keyboard navigation retain a channel and Rust/WASM evaluator for each delivered focus request.
Client memory can grow with ordinary menu use until the page is destroyed. Practical size and performance impact are
unmeasured.

## Details

`crates/farhelm-ui/src/menu_panel.rs:359–363` evaluates `return await window.__farhelmMenuFocus(...)`; initial opening
and arrow navigation use it (`775–784,821–830`). Pinned Dioxus Web 0.7.10 wraps caller code in an async function,
appending `dioxus.close()` afterward (`src/document.rs:188–195`). The explicit return skips that cleanup even for a
false result.

Its shipped `src/js/eval.js:1` stores each channel strongly in `globalThis.__channels[id]`; `close()` clears the slot.
The channel owns `JSOwner`, retaining the Rust evaluator allocation (`src/document.rs:22–32,208–254`). Joining only
awaits/deserializes the result (`dioxus-document-0.7.10/src/eval.rs:20–28,60–66`). Queue serialization limits
outstanding requests without releasing completed channels. The previous browser mounted-focus path called DOM focus
directly (`dioxus-web/src/events/mounted.rs:115–143`). Close channels on return/error while retaining result delivery.

Desktop is excluded: its completion callback closes outside caller code (`dioxus-desktop-0.7.10/src/query.rs:79–98`).
The parent independently inspected the pinned web wrapper/channel code and caller; no runtime memory measurement is
claimed.

Suggested bucket: **other**. Confidence: **confirmed static retention; retained-byte size and material degradation
unverified**. No exploit, activation, user-work loss or practical out-of-memory failure was established. Proposed drop:
none.

Possible cover: menu focus at `SPEC_impl.md:703–720`, native watchdog at `3865–3886` and bounded fleet memory at
`SPEC.md:1437–1445` do not accept per-request browser retention. Ledger page-lock fixes at `4372–4398` have different
ownership. No exact Planned, BUGS or queue match was found; ordinary persistent growth does not match the rare-glitch
filter. Provenance: `menu-focus-implementation-2/f3`, discovery pass 1, trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `menu-focus-implementation-2/f3`.
