# Attach admits a retained Gone main terminal that listings report missing

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

A request to attach can be admitted after the supervisor already knows the main terminal is gone.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:1576`.

status.rs:319-323,345-347 lists a retained Gone main handle as missing. terminals.rs:258-264 returns any retained main
handle, and handlers.rs:1563-1578 explicitly allows Gone through the attachment refusal boundary. That admission
continues to lease processing at :1788-1838. A displacing request can remove other tab attachments of this session
before the forwarder reports terminal ended (grid_stream.rs:604-620). With no held handle, terminals.rs:293-296 refuses
earlier. No race from a healthy to a gone terminal after resolution is required for this mismatch. SPEC.md:2344-2347
says the recovery pane makes no attach attempt, and the server's own attach contract at handlers.rs:1681-1705 requires
impossible attaches to leave existing attachments alone. On the public attach route, refuse already-Gone main terminals
before takeover, while preserving the separate retained handle for cleanup. Bucket: other for established erroneous
admission. A normal consumer issuing a displacing attach while another tab is attached is a separate premise for a
stronger user-visible consequence. No stream behavior observed. Confidence: source-supported branch and conditional
lease effect. Possible coverage: recovery_contract, scrollback. This is a server admission mismatch on a currently Gone
handle, distinct from stale availability in r8/r17 or missing invalidation in r19. handlers.rs:1866-1893 sends the
displaced channels their notices before :1917 queues Attached. Protocol Attach fields at
farhelm-proto/src/lib.rs:4052-4088 permit a displacing request and define Attached as a grid stream. SPEC.md:3286-3290
preserves exited content while the host is up. The exact parent fc2b0b46e6211c7ca4001d05d90221f35ee5bd97
handlers.rs:1564-1579 already admitted Gone. The new availability bit exposes the mismatch without changing Attach
itself. At the inclusive tip, core/tests/host_terminals.rs:3547-3573 asserts incumbent preservation for an unavailable
startup placeholder, not a retained Gone main handle with a sibling tab attachment. No source evidence in this pass
establishes that a current normal client issues the displacing request in that state.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r25`.
