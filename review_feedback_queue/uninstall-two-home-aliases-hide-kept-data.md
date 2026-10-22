# Two alternate home paths can hide retained data from uninstall

Reviewed commit: 0f6ad6e0d680a518b46bf43b50ff704877931216

## TLDR

Uninstall can select a folder containing data it promises to keep when the installation and data use different alternate
paths to the same home, with a nested mount visible through only some paths. The configured source chain is established;
no affected deployment or deletion was observed. Whether the home-alias exception supports this arrangement is
ambiguous.

## Details

Let `H` be the login home, and `A` and `B` be non-recursive binds of it, established before an unrelated mount at
`H/install/farhelm/data`. Let `H/.local/lib` point to `A/install`, and configured state be `B/install/farhelm/data`. The
default installation path reaches the installation through the first alias, while state uses the second. No recursive
symlink construction is required.

`crates/farhelm-helm/src/provisioning/backend.rs:2483–2513` rewrites the installation to `H/install/farhelm` while state
stays spelled through `B`. Neither raw/raw nor rewritten/rewritten comparison at `provisioning/plan.rs:820–822` sees the
state under the removed tree. Binary admission at `795–803` and path selection at `850–853` then allow removal;
`backend.rs:2663–2687` removes through the ancestor symlink. The configured state spelling survives the probe
(`service.rs:2367–2379`; `backend.rs:1948–1952`). Confirmation repeats the same plan on this stable topology
(`service.rs:1841–1864`). It does not repair the missed relationship. An alternative construction using the default unit
and a state symlink was independently traced to the same boundary.

Suggested bucket: **highest**, conditional retained-data loss. Confidence: **confirmed conditional source mechanism**.
The mounts, traversal and actual loss were not exercised. This is a residual pre-existing mechanism, not an assertion
that the raw-path repair introduced it. The initial reviewer's looping topology was corrected before retaining this
example.

Possible cover: pinned-tip `SPEC.md:3528–3536` excludes general filesystem aliasing but permits accessing home through a
second path; its application to multiple aliases and divergent nested mounts is unclear. `SPEC_impl.md:4675–4680`
specifies the paired comparisons but does not clearly accept this loss. The complete single-alias decision at
`TRIAGE_OUTCOMES.md:17567–17592` has a different trigger. No exact Planned, BUGS, queue or filter coverage was found.
Proposed drop: none. Resolve that support boundary before commissioning a general aliasing redesign.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g012/f1`, `a001/f3`, `a003/f5`, `a005/r1`. No runtime reproduction.
