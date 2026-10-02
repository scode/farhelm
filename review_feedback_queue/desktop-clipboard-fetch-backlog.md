# Remote clipboard output creates an unlimited queue of desktop HTTP requests

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

A remote terminal can submit clipboard updates faster than the desktop consumes them, accumulating pending requests and
old clipboard text without a bound.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F17 / COR-CLIPBOARD-QUEUE`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **highest**.

Anchor against the reviewed commit: `crates/farhelm-ui/src/auth.rs:431`. Recorded from the completed review without
rechecking code after rebasing onto main.

Corroborating review: security_secrets_env p1, same auth.rs producer.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: The
existing `clipboard-writes-unbounded-blocking-admission.md` and its ledger decision cover native blocking-task
admission. This finding concerns the browser producer queue. The ledger accepts a functioning-clipboard assumption and a
small native admission cap; those decisions remain in force. A native refusal may reduce the producer backlog, so check
the remaining scope during implementation rather than assuming a second mechanism is mandatory.

Every remote OSC 52 clipboard sequence causes the desktop page to submit a new HTTP request containing the clipboard
text. The terminal parser immediately treats the sequence as handled, while the native endpoint serializes actual
clipboard writes through a mutex. There is no limit on outstanding submissions and no replacement of obsolete pending
values. A remote terminal producing clipboard updates faster than the native clipboard accepts them can therefore
accumulate requests and their encoded text indefinitely. Browser connection limits constrain concurrent transmission,
not the number of requests waiting to be sent.

Remote sessions are allowed to replace the clipboard, but sustained output must not exhaust the desktop webview's memory
or keep issuing old copies long after the terminal closes. Bound the browser-side bridge, for example to one active
request and one replaceable latest pending value. Keep terminal parsing nonblocking and preserve silent best-effort
failures. Test a burst against a fetch that never settles and require retained and submitted work to remain bounded.
This complements the already queued native admission issue: adding an awaited limit only at the Rust endpoint would
still leave the browser backlog.
