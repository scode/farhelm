# Mixed input acknowledgements can report discarded bytes as written

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After a transient Linux terminal-write allocation failure, input discarded by the terminal host can be reported as
written when a later input succeeds before their acknowledgements are sent. The conditional source route is established;
production prompts are disabled, and no user-work loss is claimed.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
static route on Linux; runtime occurrence and frequency unmeasured

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:3744-3763.

a059/f11 supplies the source-complete allocation-failure/coalescing producer missing from a062/f1 and a061/f3. Parent
personally checked the official immutable Linux source and host/runtime/client paths. Preserve all earlier
investigated-uncertain histories and the narrow original g130/f1 aggregation refutation. The auditor suggests high
through false prompt success, but helm agent_requests.rs:333 disables production agent prompts. Record other for
established wrong input-fate accounting; a supported consequential prompt/work-loss route would change severity. No
additional global defect is counted.

At the reviewed tip, crates/farhelm-supervisor/src/hosts/terminal.rs:3738–3763 assigns cumulative acknowledgements
Written-first. crates/terminal-host/src/host.rs:581–621 discards pending input on a non-EAGAIN/non-EINTR write failure
without closing pty_open, while :799–803 permits later input and :1094–1117 replaces an unstarted final acknowledgement
with the new totals. Official Linux v6.8 drivers/tty/tty_io.c:967–1048, especially :997–1010, permits ENOMEM during
write-buffer allocation without closing the live tty. Later ordinary input can succeed once allocation succeeds. A prior
ordinary output burst can leave the client socket temporarily unwritable with the userspace queue below its high-water
mark. Input A is discarded; a later B succeeds before the unsent acknowledgement starts. The host polls incoming input
while output waits (host.rs:269–410,987–1054). The supervisor client permits several small in-flight inputs within 256
KiB (hosts/client.rs:175–184,852–864,1008–1075). For lengths a<=b, cumulative written=b and discarded=a passes
whole-Input endpoint validation at client.rs:583–615 while the runtime labels the earlier A Written, although it never
reached the program. The source-complete producer is transient kernel allocation failure, not an invented
terminal-reopening program. Exact buffers, OS and timing are conditional; no runtime reproduction or frequency
measurement was performed.

Possible coverage and caveats: none found. terminal-host SPEC.md:387-426 requires ordered accounting and later discard
on a non-full write failure, not false success; queue/filter rules do not accept false success or work loss. Searches of
SPEC.md, SPEC_impl.md, TODO Planned, unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md found no exact
accepted mixed-outcome misattribution.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `a062/f1`, `a061/f3`,
`a059/f11`, `g130/f1 (narrow original refutation)`. Original append-only events, revisions, withdrawals and independent
judgments are retained in that run's catalogs. Locations refer to the named reviewed revisions; the original revision
context is ca9b0a19856dc136f63ad2d14f1076027edce1eb; introduction 70cd68a03c101493e0adbf00bb27bb8004adfe41. Verdict
AGREE with the narrow source-refuted drop.. This was a source-only review; runtime occurrence and frequency were not
measured.
