# Unscoped tab logging says scoped

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A tab running without a system scope can still be logged as scoped merely because a candidate unit name exists.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The diagnostic derives scoped=true from the name rather than the actual Unscoped marker. The managerless/macOS path is
ordinary enough that rarity is not established for filtering. This is pre-existing diagnostic correctness, without an
ownership or process-kill defect.

Source locations: `crates/farhelm-supervisor/src/service/core.rs:14132-14135`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

New independent a018/d2 discovery, revised without erasing its initial event. Reviewed
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: a normal tab opened on a host without usable systemd containment can be
logged as scoped even though it uses only portable process cleanup. This misstates the containment available when
investigating tab termination/cleanup. Source chain independently verified: scope.rs:184-187 derives a candidate unit
from UUID syntax only; launch_prefix :895-924 returns None when no usable manager/tools exist. core.rs:13898 mints the
tab UUID, :13904-13923 retains Some(unit) but uses an empty prefix after unavailable-manager fallback; :13929-13933
correctly selects Unscoped and :14015-14037 writes that correct marker. Successful-open log at :14134 instead records
scoped=unit.is_some(), so nameability is incorrectly reported as actual scope selection. SPEC.md:1489-1501 explicitly
supports manager-less/macOS portable cleanup, and SPEC_impl.md:3709-3717 distinguishes the per-open selection from mere
unit naming. Hence this is an ordinary supported trigger, not a rare interruption; FILTER.md:24-42's rarity condition
does not fully match. No wrong kill target, corrupted marker or false user-facing open success is alleged. Suggested
bucket OTHER; confidence confirmed source mismatch, no runtime reproduction. proposed_drop=none.
possible_cover=TODO.md:514-540's future Linux manager requirement does not suppress this: it is not Planned, does not
describe this log field, and keeps macOS portable cleanup. TRIAGE_OUTCOMES.md:6083-6104 concerns skipped scope
enforcement on Delete, not this diagnostic. Searches across all required coverage sources found no exact separate cover.
IMPORTANT provenance: immutable exclusive-base source da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:core.rs:13802-13805
already contains the same field, so this is pre-existing, not introduced by fb81031deb85a5fb9f1110b4aa9dbbe238f048af or
the reviewed optional-PID fix. It was discovered while exploring tab-open/failure scope decisions in this assigned
lifecycle/diagnostic scope. A minimal repair would log actual tab_scope, but no fix was made or requested. No unresolved
harmful consequence or runtime-duration claim.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a018/d2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
