# Stop or Restart can skip a surviving process after terminal loss

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

Stop can report success or Restart can start another run while the prior program survives after its terminal host
disappears.

## Details

**high.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:867`.

tip hosts/terminal.rs:4037-4047 and :4133-4148 change the runtime to Gone without clearing its child pin; :823-835
returns Gone without checking that pin. handlers.rs:867-912 derives a root only for a live probe PID, and
:945-954,1120-1142 sends Gone into rootless Stop. An unscoped launch still runs the portable sweep
(sweep.rs:1321-1327,1618-1629), which can miss identities for which the dropped anchor was the only usable ownership
evidence (sweep.rs:473-493,1171-1185). hosts/terminal.rs:759-789 documents the still-live case and shows Delete's
independently validated pin, used in teardown.rs:320-345. E2E host_terminals.rs:283-332 asserts that Stop walks an
unmarked tree only while the host still answers. A recorded scope whose manager is unavailable separately makes cleanup
fail. An unscoped process with no pin, identifiable ancestor, or readable marker stays within
SPEC.md:2248-2258,2272-2279's accepted limit. A held readable pin is existing evidence, even when host liveness is gone.
Query it for authorized Stop and propagate unreadable evidence, while checking consent independently (l9). This shares
the dropped-root defect with recovery r6, with false Stop completion instead of overlapping relaunch. It is separate
from l4's skipped walk after a confirmed exit. Bucket: high for false completion of process cleanup through a
source-established omitted known root. Remaining test gap: surviving pinned tree after Gone with neither scope nor
usable markers. No runtime observation. Confidence: confirmed by source for the omission. proposed_drop: merge into
recovery r6 if the combined finding retains the separate Stop entry point, consequence and consent check. Possible
coverage: recovery r6, lifecycle_e2e, lifecycle_unit.

Independent check of the proposed exclusion. AMBIGUOUS via fc2b0b46e6211:handlers.rs:865–894,
hosts/terminal.rs:757–786,4035–4049,4131–4150, and SPEC.md:2224–2232,2246–2260. The excerpts support the distinction
between host liveness and a verifiable child identity. The merge still lacks retained recovery/r6 text preserving
authorized Stop's separate entry point, possible false completion, and independent consent check.

Additional source record at `063203912059cb23101d9a458d8821f02f1bc18f`,
`crates/farhelm-supervisor/src/service/core.rs:12494`.

hosts/terminal.rs:4133-4148 marks the runtime Gone after reconnect, and :823-835 turns Gone into a successful probe
without a PID. core.rs:12494-12509 keeps no root. Host_restart.rs:536-568 and :584-587 skip the retained host's
generation/socket, and core.rs:12571-12598 sweeps only outstanding roots and the row's scope before launching. On an
unscoped host, sweep.rs:473-493 and :1171-1185 show how missing markers can leave that child and its family without any
seed. hosts/terminal.rs:759-789 expressly covers a still-identifiable pinned child that outlives its terminal host, and
teardown.rs:319-344 already uses that identity for Delete. hosts/client.rs:640-646 and :1221-1239 normally keep a
discoverable live socket owner classified as unavailable. This finding does not assume a missing pathname always hides a
still-running host. A real host exit can leave its child alive because host ending sends a hangup and does not wait
(crates/terminal-host/SPEC.md:149-155). status.rs:319-323 makes the retained Gone state eligible for the recovery card.
SPEC.md:2226-2230 and :2248-2255 promise cleanup of ordinary descendants Farhelm can identify, and :2272-2279 requires
refusal on unconfirmed cleanup. SPEC_impl.md:1817-1820 describes the runtime's stricter live-report root rule and
:2159-2162 explicitly makes a pinned-identity exception for Delete. That implementation rule explains the omission and
should be reconciled with the product guarantee on Restart. Recheck the retained pinned identity as a separate cleanup
anchor, and distinguish terminal availability from process liveness. Bucket: high for potential overlapping runs in one
workspace through a source-supported omitted root, limited to a still-live, readable identity outside both markers and
scope. No loss-of-work consequence established. Confidence: the omitted root is confirmed by source, with survivor
conditions specified by the host contract. No observed runtime outcome. Possible coverage: stop, recovery_contract.
Parent comparison: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97 core.rs:12505 and :12585-12598 already had the root
omission. The reviewed commit makes retained Gone handles report the missing terminal and brings the omission into the
recovery flow. It did not introduce the root-selection code. Synthesis with Stop/l8: the unscoped reaper does run.
ScopeUnits::recorded(None) sets needs_sweep (sweep.rs:1321-1327). The only initial roots in this call are validated
outstanding-host children and matching environment markers (sweep.rs:1495-1504,1524-1531,303-311,639-672,1180-1185). The
PPID walk expands down from those roots. A marked descendant can be reaped but cannot rediscover its unmarked parent. An
independently rooted ancestor or an eligible marker on the held child can substitute in a particular run. The held
record is skipped, and the discovery of its host process would not itself be a root. With no such substitute, a
still-pinned running child and the unmarked portion of its family receive no cleanup signals, and an otherwise
successful empty sweep permits the replacement to launch. Host shutdown is later and confirms the host only. Keep
Restart overlap here and Stop false completion/consent in Stop/l8. One combined repair can reuse pin validation only if
both boundaries are preserved. Neither is the skipped scoped walk tracked in r11/Stop l4. Existing assertion at
crates/farhelm/tests/e2e/host_terminals.rs:388-402 checks that an adopted, answering host provides a root and the old
family is gone when the replacement begins; :466-489 checks the live parent relation and that host hangup is
insufficient for the descendant. It never asserts the same cleanup after the held runtime reports Gone. This is
assertion evidence only.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/l8`, `recovery/r6`.
