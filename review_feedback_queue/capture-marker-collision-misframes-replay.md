# Exact capture-marker collisions can truncate replay and copy text into logs

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A terminal line that exactly matches a live tmux reply marker can be mistaken for a reply boundary. Depending on the
following text, attachment can fail, replay can silently omit history, or preceding terminal contents can enter
diagnostics. A practical collision and disclosure to an unauthorized reader have not been demonstrated.

## Details

tmux 3.7c sends captured text without escaping embedded protocol-looking lines
([capture implementation](https://raw.githubusercontent.com/tmux/tmux/3.7c/cmd-capture-pane.c)). Reply markers contain
wall-clock seconds, a global incrementing command number and flags
([marker generation](https://raw.githubusercontent.com/tmux/tmux/3.7c/cmd-queue.c)).
`crates/farhelm-supervisor/src/tmux/control_codec.rs:368–381` accepts an exact matching `%end` or `%error` line as the
current boundary. Historical transcript lines with different numbers remain payload.

A lone matching `%end` finishes capture early and normally makes the next reply read fail at the real terminator. A
captured suffix containing two additional complete fake reply blocks can instead satisfy the visible-snapshot and
cutover reads (`tmux/stream.rs:1115–1138`) and return truncated history successfully. The reader accepts any next
`%begin` identity. Remaining captured text can reach the live notification classifier. This does not let captured text
change the capture targets that were already submitted.

An exact `%error` collision embeds preceding capture bytes in the error (`tmux/control_codec.rs:377–380`). Catch-up
propagates it through `tmux/stream.rs:1113–1120` into Display-valued supervisor diagnostic fields
(`service/connection.rs:1368,1679–1681`). The pinned tracing formatter's ordinary named-field branch does not sanitize
those Display values. Initial attach errors logged by the helm use capped, Debug-escaped `peer_text`
(`crates/farhelm-helm/src/terminal.rs:409`), which refutes that particular terminal-control injection route. Escaping
does not establish confidentiality of the copied content. Catch-up detach text reaches the existing viewer.

Suggested bucket: provisionally **highest**, retaining the unresolved confidentiality question. The demonstrated
conditional replay/diagnostic consequence alone is `other`, as the producers noted. Confidence: **confirmed parser and
logging mechanism; unverified practical collision and confidentiality impact**. Exact current numeric identities and
terminal styling constrain the trigger. No unauthorized new reader, foreign-pane capture, cross-host disclosure,
credential disclosure, process loss or added terminal capability was established. No runtime reproduction was run.

Possible cover: SPEC_impl.md:1378–1387 requires failures instead of incomplete successful replay, so the multi-block
variant is not an express acceptance. The rare-glitch filter may cover a lone collision with only a safe attachment
failure or imprecise diagnostic; it does not automatically cover successful incomplete replay or an unresolved security
effect. SPEC.md's local-authority rule may cover deliberate same-account construction, while expressly retaining
protections against agent output and remote-host effects. The ledger's
`remote-attach-errors-inject-terminal-controls-into.md:7207–7225` covers the fixed helm escaping route, not capture
framing or confidentiality. `unbounded-aggregate-capture-replies.md` concerns memory allocation, a different
consequence. No exact Planned, BUGS or current queue match was found.

Proposed drop: the stream producer proposed dropping the security claim absent demonstrated disclosure and assessing the
residual filter match. Independent fresh-context disposition review 4 said KEEP: no practical collision or unauthorized
reader was demonstrated, but neither exact acceptance nor a complete confidentiality refutation was established.
Unambiguous capture framing and avoiding capture bodies in parser errors are possible repair directions; triage still
needs the practical trigger and destination-reader assessment.

Provenance: `supervisor-terminal-control-1/f4` and `supervisor-terminal-control-2/f2`, latest `turn2.result.md`;
`drop-review-4/verdict.md` KEEP for control-2 `findings.jsonl:6`. One underlying capture-framing issue, retaining both
the incomplete-success and content-in-diagnostics consequences.
