# Launch-input duplication rationale overstates database storage

Reviewed commit: a2c2f8861418f8222eca096c797b9f31585ecc1a

## TLDR

The implementation specification says launch input contains nothing absent from the session database, but create-time
agent-request prompts are an exception. Retaining the private current-generation file is explicitly allowed.

## Details

`SPEC_impl.md:3433–3435` claims complete duplication. At production commit `4f6897755f0d714e282c6ab06e6b197a1b8495ab`,
`crates/farhelm-supervisor/src/service/core.rs:1703–1724` stores only the agent-request prompt hash in its reservation,
while raw text enters launch input at `14443–14519`. The shipped create-time producer is
`crates/farhelm/src/main.rs:943–962,1033–1040,668–691` through helm `agent_requests.rs:1757–1759,1831–1859,2597–2632`.
The disabled Prompt operation affects already-running sessions, not these create-time prompts.
`service/launch_artifacts.rs:571–575` repeats the duplication rationale.

Suggested bucket: `other`. Confidence: confirmed storage/rationale distinction. No new reader, cross-account exposure,
prohibited lifetime or post-Delete retention was established. Possible cover: `SPEC_impl.md:3438–3440` and ledger
`TRIAGE_OUTCOMES.md:1641–1705` allow the whole current-generation file until Delete, including prompt payload. That
settles the retention allegation but does not make the duplication explanation accurate. Ordinary keyed creates can
store raw reservation fingerprints, so do not generalize hash-only storage to every prompt. Provenance: `a007/f1`,
a015's additional comment instance. No runtime reproduction ran.
