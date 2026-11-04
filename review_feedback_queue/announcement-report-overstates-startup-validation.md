# Table validation does not exclude announcement startup failures

Reviewed commit: c5ae8548439cc5bfa33c11ef7ce8da6129d8072a

## TLDR

The announcement landing report says startup refusal requires an invalid compiled entry. Database and creation-version
errors can also stop startup, even when that table is empty and valid.

## Details

Suggested bucket: **other**. Confidence: **confirmed source route**. Proposed drop: none. Possible cover: none found.
This is a limitation in the report's explanation, not a claim that propagating database failure is a new product bug.

`plans/reports/in-app-announcements.report.md:127–128` says startup refusal "today ... needs an invalid table entry".
`crates/farhelm-helm/src/lib.rs:2266–2269` propagates `enqueue_builtin` failure before serving. On tagged builds,
`release_announcements.rs:45–55,85` reads and parses the recorded creation build and calls the database enqueue even for
an empty table. `announcements.rs:69–80,86–116` propagates query, transaction and commit errors. Builds from main return
before these producer steps at `release_announcements.rs:46–47`.

SPEC.md, "Announcements", and SPEC_impl.md's GUI announcement contract require enqueueing before serving; they do not
make those database operations infallible. Narrow the report's sentence to invalid compiled content, or include the
other refusal sources. Existing startup database checks may fail too; no runtime availability regression is asserted
here. The ordinary healthy-filesystem acceptance does not make the report's exhaustive factual claim true.

Targeted checks of the specifications, empty Planned/unlanded queues, BUGS.md, review index and triage ledger found no
matching disposition. No filter clearly covers this report correction. Provenance: review run
`ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g040/g040-l2`. Source-only; no database failure was injected.
