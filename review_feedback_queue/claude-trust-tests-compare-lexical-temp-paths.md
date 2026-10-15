# Trust tests compare canonical paths with unresolved temporary paths

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

The new trust tests can fail on macOS despite correct trust behavior when the temporary directory has a symlinked
ancestor.

## Details

Suggested bucket: `other`. Confidence: confirmed by static path tracing under the symlinked-root premise; actual macOS
execution was not run. Source: separate adversarial trailing-review agent, Claude trust scope. Proposed drop: none.

`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:298–311` compares `resolve()`'s canonical configuration path
with a lexical path derived directly from `tempfile::tempdir()`. The launch test makes the same comparison at
`crates/farhelm-supervisor/src/launch.rs:3565–3567,3597`. With a symlinked temporary root, such as `/var` resolving
through `/private/var`, those spellings differ even when they name the same file.

The trust round-trip fixture also seeds a project key using its unresolved directory spelling at `trust.rs:301`.
Mutation uses the canonical spelling. Removal then preserves an additional empty canonical project object, making the
final comparison at `:329–332` fail for the fixture's unrelated path assumption. Canonicalize the fixture root before
deriving configuration paths and project keys; assert that premise before the trust operation.

Possible cover: `file-stat-test-compares-unresolved-temp-path.md` concerns a different fixture. No matching Planned
item, accepted bug, ledger decision, or filter was found. This is test portability, not a demonstrated shipped-product
defect.

The complete introducing patch and the affected fixture bodies were read. No runtime test was run and no test-process
environment was changed.
