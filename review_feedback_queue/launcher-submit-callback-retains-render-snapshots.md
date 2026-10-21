# Launcher submit callbacks retain render snapshots

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

Keeping the launcher open while editing or receiving updates accumulates retained submit callbacks and their captured
data. Closing the launcher releases them; practical memory growth has not been measured.

## Details

Suggested bucket: **other**. Confidence: **confirmed allocation and retention lifetime**, with no measured slowdown,
out-of-memory failure or user-work loss.

`crates/farhelm-ui/src/list/create_form.rs:5295–5301` constructs `Callback::new` in the component body and captures
host, source, prefill, API and catalog snapshots. Matching Dioxus 0.7.10 `events.rs:484–496` explicitly warns that these
scope-owned allocations survive until the component drops. This callback is never released; dropping obsolete listener
or effect handles does not release the owner's allocation. Use a hooked callback with the intended update lifetime
rather than allocating another scope-owned callback on every render.

Before extraction, parent `667b8f06d567455b115c0f158e5b954ea19209fa:create_form.rs:5214` supplied its closure directly
to the native submit listener. Dioxus `nodes.rs:772–777` uses `ListenerCallback::new`, whose implementation at
`events.rs:600–657` retains an ordinary reference-counted closure. This distinction makes the new submit snapshots an
additional retention source, even though other older component-body callbacks have similar problems.

Possible cover: none found in the required coverage sources. The existing evaluator-channel retention item has a
different allocation and lifetime; ordinary repeated renders are not a rare-glitch trigger. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g046/f3`, following `g047`'s interface lead;
the matching dependency source was checked independently by the parent.
