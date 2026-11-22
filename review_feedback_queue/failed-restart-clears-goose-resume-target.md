# Failed named Goose Restart can clear its saved target without reporting the failed restore

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A failed named Goose Restart can leave the saved resume target cleared if Farhelm also cannot restore the previous
outcome. The client only sees the launch error.

## Details

**highest.** Source inspection.

crates/farhelm-supervisor/src/store.rs:4704–4764,4792–4864,5343–5369;
crates/farhelm-supervisor/src/service/core.rs:7025–7060,12630–12644,12778–12795,12958–13007,13097–13102;
SPEC.md:2275–2289,3962–3972,4763; SPEC_impl.md:8642–8649, 1a53d9d35889d7dde96e31320cc1681dd013737d

A named Goose restart clears the old captured ID, report source and ownership version in the committed
generation-opening transaction because that kind of resume cannot report a replacement. PriorRun retains the old triple
in memory. If a definitive pre-spawn failure is followed by a failed abort_relaunch write, the triple remains cleared
and plain Restart loses its saved target. The API returns only the original launch error. The generation-fenced restore
write could have restored the triple, prior outcome (including exit code, annotation and error detail), terminal
address, terminal kind, scope selection, boot finality and hook setup fields. It never rewinds the generation or
restores Codex/OMP per-run attribution. Ordinary Stop can record a new outcome; reload that rediscovers the previous
terminal can additionally record its address and exit code if available. The transition writes only outcome columns and
optional address, so neither route recreates the original capture/provenance, previous annotation or error detail,
scope/kind/boot selection or hook fields through that transition. A known selector can still be used with Restart with,
and no vendor conversation is deleted. SPEC.md:3967–3968 says definitive pre-spawn abort restores the Goose capture.
SPEC_impl.md:8647–8649 allows the mirror update only after durable success. SPEC.md:2275–2289 explicitly accepts a
temporarily wrong capture that can resume a different conversation under coinciding rare faults, and does not expressly
name the absent saved target; coverage by analogy remains a commissioner judgment. The reviewer suggested high for the
conditional loss of a saved resume action, other for hidden diagnostic/status alone. Confidence confirmed conditional
source route, with no runtime incidence asserted. Proposed_drop no. Possible coverage: the coinciding-fault provision by
analogy only.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `implementation-contracts/IC-13`.
