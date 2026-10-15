# Trust edits can change unrelated numbers in Claude's settings

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

Turning on checkout trust can silently change numeric values elsewhere in Claude's settings file.

## Details

Suggested bucket: `highest`, conservatively for unintended mutation of vendor state. Confidence: the numeric
transformation is confirmed; a consequential affected decimal occurring in real Claude configuration is unverified. No
personal configuration was inspected and no credential or practical accounting loss was demonstrated. Source: separate
adversarial trailing-review agent, Claude trust scope. Proposed drop: none.

`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:229–230,259` parses the whole vendor document into
`serde_json::Value` and serializes it again. The workspace uses `serde_json = "1"` at `Cargo.toml:202`; read-only Cargo
feature inspection found neither `float_roundtrip` nor `arbitrary_precision`. The default parser does not preserve every
binary64 value. A standalone offline probe with serde_json 1.0.152 rewrote `0.9176884786531495` to `0.9176884786531496`.
Node confirms these are distinct numbers, not equivalent spellings. The coordinator reran the probe and numeric
comparison with the same results.

This is deterministic mutation without another writer, contrary to `SPEC.md:823–829` and `SPEC_impl.md:1877–1883`, which
limit the edit to the checkout's trust key and preservation of unrelated state. Both granting and removing trust use
this whole-document path. Preserve unrelated numeric values, or refuse a document that cannot be preserved. Add a
round-trip case containing a decimal that distinguishes the two parsers; checking only integer fixtures misses it.

Possible cover: the expressly accepted unlocked exit-save race and future vendor-format changes do not cover a
deterministic rewrite with the current parser and no concurrent writer. No exact Planned item, BUGS entry, existing
queue item, ledger decision, or filter was found.

The complete introducing patch was read. Runtime evidence was limited to the isolated parser probe and Node comparison;
no real vendor state was changed. Surrounding code was inspected selectively.
