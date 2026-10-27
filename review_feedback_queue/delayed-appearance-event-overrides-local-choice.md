# Delayed appearance notification overrides a newer local choice

Reviewed commit: afb41f43c8ea35c3a7333e06045ad11cf1702534

## TLDR

With two Farhelm pages open in the same browser origin, an older notification can undo the theme just selected in one
page. That page's palette and Settings radio can then disagree with the saved choice.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional source mechanism under the HTML event model**; actual
Chromium/WebKit scheduling and frequency were not measured. Durable storage remains correct. No credential, process,
wrong-target or user-work-loss consequence is established.

Page B selects Dark, queuing a storage notification to A. Before delivery, A selects and saves Light. A then applies B's
captured Dark value without reading current storage (`crates/farhelm-ui/assets/appearance.js:64–80`). A's own write does
not notify itself, so it supplies no later correction. The in-memory owner survives Settings remounts
(`25–26, 41–44,81–82`), and explicit Dark ignores system changes (`58–59`). A fresh choice or full page reload repairs
it. The same source remains at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`.

The [HTML storage broadcast algorithm](https://html.spec.whatwg.org/multipage/webstorage.html#the-storage-interface)
queues notifications carrying captured values. Its
[event-loop model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model) permits selection
among task queues. The inference is that those rules do not guarantee delivery of the older storage task before newer
local user input. This is a permitted conditional schedule, not an empirical browser observation. The browser test
serializes external updates before proceeding (`e2e/tests/appearance.spec.ts:372–423`).

Possible cover: the rare-glitch filter might cover this if its refresh boundary includes deliberate full page reload.
Ordinary application refresh/reconnect or Settings remount does not repair it; the sub-second race filter also needs an
unestablished timing bound. No exact specification, Planned, BUGS, queue or ledger coverage was found. Proposed drop:
none while filter coverage is ambiguous. Reconcile storage notifications with the current stored choice before applying.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g037/f1`; `g038` corroborated
the source mechanism.
