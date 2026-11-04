# Direct takeover leaves program-drag guidance active on the detached screen

Reviewed commit: 4e39472e73711eba694854e921b1f77459092188

## TLDR

After another client takes control, the retained terminal can still say its program handled a drag that the detached
client cannot send. Both guidance queued before takeover and guidance from a later drag can appear.

## Details

The grid guidance driver in `crates/farhelm-ui/assets/terminal.js:5394–5458` checks its private active flag,
mounted/alive state, press sequence, program-copy count and selected pane. It does not check decision detach, socket
state or takeover.

A direct takeover at `terminal.js:5030–5060` calls `latchTakeover:3311–3370`, which unmounts connecting/recovering
islands but retains already-open ones. `socketEnded:5180–5209` marks the connection ended without stopping guidance, and
`sync:3156–3190,3240–3243` preserves the same island. Its alive flag is cleared only by disposal at `4920–4925`. These
surviving checks allow a pending grace callback or later plain drag to raise guidance on the detached screen.

The recovery-restore test at `e2e/grid-client/grid-attach.spec.ts:748–814` exercises a different route. Reconnect
unmounts at `terminal.js:1875`, stopping the driver before freeze at `6402`; that teardown does not establish silence
for direct takeover. The plan's teardown-only reasoning at
`4f88e7d6a781de15a965083fb80379c0c7bbfb1b:plans/queue/grid-drop-and-drag-notice.md:114–117` misses the already-existing
retained-island route. The report at
`7a7c160b124ac270f9514642563080dcf36bce44:plans/reports/grid-drop-and-drag-notice.report.md:17–18` overgeneralizes
silence to takeover screens.

`SPEC.md`, “Terminal experience,” requires a retained non-live takeover snapshot. `SPEC_impl.md`, “Terminal widget:
xterm.js island,” 1653–1658 says that snapshot remains selectable without raising guidance. Stop/hide guidance at
decision detach or gate its ownership on a live attachment, while preserving frozen-screen local selection and copying.

Verify direct two-client takeover both while a grace callback is pending and before a subsequent drag. The hint should
remain absent on the detached screen without disabling its local copying. This is proposed verification, not an executed
reproduction.

## Confidence and priority

Confidence: confirmed source path, without runtime reproduction. The initial unmount/attachment-disposal refutation
remains valid on that narrower route. The involved assets remain unchanged through
`c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Suggested bucket: `other`, for misleading guidance. No data-loss or security route is established.
`could_be_highest: false`. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g055-l3` is the executable finding and `g039-l2` supplies plan/report evidence for this same
direct-takeover defect.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`: owner `g055`, ID `g055-l3`, latest `g055/findings.jsonl` physical
line8; owner `g039`, ID `g039-l2`, latest `g039/findings.jsonl` physical line14. The latter record adds plan-time
retention and exact report locations while preserving the same product mechanism.
