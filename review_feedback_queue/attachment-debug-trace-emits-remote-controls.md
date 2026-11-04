# Attachment debug traces emit remote-controlled terminal bytes

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

With attachment debug tracing enabled, a remote tab identifier can put terminal control bytes into the operator's log
display. This is a conditional trust-boundary issue; no payload or runtime exploitation was attempted.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route, debug gate required

Source locations: 58094288259b0c29f74ea25936d4fe006c72128f:crates/farhelm-helm/src/terminal.rs:133,140-145,153-158;
unchanged at ca9b0a19856dc136f63ad2d14f1076027edce1eb.

Preserve the exact raw-formatting caller and hostile-supervisor boundary from the primary and independent evidence.
Tooling-only smoke credential acceptance does not cover production attachment tracing. Do not claim an observed command
execution or universal effect on all log consumers.

TabInfo.id is an opaque deserialized String (crates/farhelm-proto/src/lib.rs:1747-1757); qualify_session_info leaves
tabs untouched (helm sessions.rs:795-807); UI Tab is the same protocol type (ui lib.rs:279-282,499-500). The shipped GUI
mounts the first reported tabs concurrently (tip session_view.rs:1628-1639,1680-1696), percent-encodes exact ids into
terminal URLs (tip tabs.rs:256-264 and api.rs:1746-1750), and the helm Query decoder preserves the value with no shape
validation (terminal.rs:198-204,273-277). After Attached, TracedAttachment stores tab:<raw id> and logs terminal = %...
on start and Drop (terminal.rs:133,143,156; guard at 772-775). Both Bytes and Grid stream kinds reach this trace. The
target must be enabled at debug (terminal.rs:99-111), either specifically or through a broader RUST_LOG filter; ordinary
info logs do not emit these events. The normal binary installs
tracing_subscriber::fmt().with_env_filter(...).with_writer(stderr) (main.rs:2616-2624). Tip
Cargo.lock:6510-6518,6531-6547 pins tracing-core 0.1.36 and tracing-subscriber 0.3.23. Version-matched
DefaultVisitor::record_debug guards the message field only; named fields directly format value (subscriber
fmt/format/mod.rs:1299-1338). DisplayValue forwards raw Display (core field.rs:719-736). Normalize the terminal field
with manager::peer_text; preserve ordinary trace values.

With attachment debug logging enabled, a compromised remote host can emit terminal control sequences into the local
operator's logging terminal by listing a tab and accepting its attachment.

Possible coverage and caveats: tip TRIAGE_OUTCOMES.md:6927-6942 and 7188-7221 fix older linger-stderr, parse-error and
attach-refusal logs; their triggers and code sites differ from this successful-attachment trace, so they are precedent,
not duplicate coverage. SPEC.md:3781-3893 preserves the remote boundary and only explicitly allows OSC52 writes through
the remote program's terminal display, not arbitrary control-bearing diagnostics; SPEC_impl.md:2950-2959 and 3132-3144
likewise preserve effect-scoped trust and escaped diagnostics. Deliberate local debug configuration does not waive that
remote boundary (SPEC.md:3792-3809). TODO Planned is empty; no matching BUGS, queue or unlanded-plan coverage found.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g038/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 58094288259b0c29f74ea25936d4fe006c72128f;
verified persistent at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence and
frequency were not measured.
