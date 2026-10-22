# Uninstall plan normalizes containing paths independently

Reviewed commit: aedca923ea52b4a3ef31fb4690002346af5a5126

## TLDR

The proposed home-alias uninstall design can miss that retained Farhelm data lives inside the directory it will remove.
With a particular bind-mount and symlink layout, implementing that design can delete data the confirmation promises to
keep. This finding concerns the committed plan, not its later implementation PR.

## Details

Suggested bucket: **highest**, for removal of retained host data. Confidence: **confirmed source-level counterexample to
the proposed algorithm**, conditional on the layout below. No runtime reproduction or frequency measurement was
performed; no source premise remains missing for that layout.

`plans/queue/uninstall-aliased-home.md:59–69,159–177` independently rewrites each canonical path through a home alias,
checking full-path identity before accepting each rewrite, while retaining the existing containment checks. Let `H` be
the login home and `A` a non-recursive bind alias of it. Let `H/.local/lib` link to `A/installs`. A differing nested
mount at `H/installs/farhelm/data`, absent under `A`, makes the lib directory rewrite to `H/installs/farhelm` while its
contained state falls back to `A/installs/farhelm/data`. The binary has no divergent nested mount and normalizes beneath
the lib directory, so its check passes.

At the reviewed revision, `crates/farhelm-helm/src/provisioning/plan.rs:804–812` checks state containment textually and
misses this mixed spelling. `733–744,833–836` retains the original removal path. `service.rs:1375–1492` supplies the
facts, and `backend.rs:2594–2608` removes through that original symlinked parent, reaching the data under `A`. Before
the proposed rewriting, both canonical paths used `A` and containment refused removal. Replanning against the same
stable topology repeats the error.

This does not require a test-only lib override or a final-component symlink. The real setup reads the login home
(`backend.rs:2121–2186`), and existing tests explicitly allow symlinked parents and remove the real tree beneath them
(`crates/farhelm-helm/src/provisioning.rs:6357–6404,6660–6675`). The assigned delivery report at
`a0db2eb64f63cac6dc70957d8937271189719fd4`, `plans/reports/uninstall-aliased-home.report.md:19–24`, discloses that
differing nested mounts were not tested. It is not evidence that a later implementation has this defect.

Possible cover: pinned `SPEC.md:3498–3503` and `TRIAGE_OUTCOMES.md:4099–4111` generally accept filesystem aliasing. The
plan explicitly adds a home-alias exception and discusses non-recursive binds (`101–108,130–136`), so that broad
acceptance does not unambiguously settle this newly proposed deletion. No exact Planned, BUGS, queue, ledger or FILTER
coverage was found. Proposed drop: none. Preserve containment relationships across normalization, rather than treating
independent leaf-identity checks as proof that those relationships survive.

Recording-time main `73f2bb9007c2c6b9fa9eee7f67f1f79a881abaff` adds an explicit home-alias exception to `SPEC.md` and
describes independent full-path identity-guarded rewrites in `SPEC_impl.md`, while still requiring state outside the
removal directory. These changes are additional possible coverage for the chosen algorithm, not explicit acceptance of
deleting retained state through mixed spellings. The later implementation landed as
`ac1cec356f5b4e64981f8a019f402f0ed447b919`, outside the pinned review range. Its implementation is not claimed reviewed
by this plan finding.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g008/f1`, independently retained by `g049`.
