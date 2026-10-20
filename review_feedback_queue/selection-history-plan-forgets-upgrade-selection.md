# Visit-history plan conflicts with preserving the remembered selection

Reviewed commit: 592d8c5a5a6409a193500c474a05f3a11c4607b5

## TLDR

Following the session-history plan literally would make the first launch after upgrading open a different session and
overwrite the user's remembered selection. This is a conflict in an unimplemented plan, not an observed product bug.

## Details

`plans/queue/select-newest-on-close.md:32–47` requires history's head to equal the remembered selection and replaces the
fresh-load choice with the first listed history entry, falling back to newest-created. But `66–69` requires existing
installations to start with empty history. If the remembered session is A and newer-created C exists, that first reload
therefore chooses C. The new automatic-pick write (`54–58`) then replaces the remembered A with C before any user visit.

The history being empty is explicit; resetting the already-known selection is not explicitly accepted. The recorded user
decisions (`78–89`) concern visit history and remembering automatic picks, while the binding constraints (`93–98`) still
name restoration of the remembered session. The current selection is available independently of the new history. Seed
history from that value or preserve it during the empty-history transition, reconciling the head invariant. Add an
upgrade case with a valid remembered A and a newer-created C before implementing the choice.

Suggested bucket: **other**; selection preference, with no user-work loss or action on the newly selected session
established. Confidence: **confirmed inconsistency in the plan**; the behavior is a literal-execution prediction. No
implementation or runtime validation of this plan was performed. The executor remains bound by the specification; the
conflict is something to resolve before execution, not a claim that it must ship.

Possible cover: the empty-history criterion above could reflect an intended reset, but it does not explain that
consequence and conflicts with the plan's restoration premise. SPEC.md's shared-selection contract and the plan's
binding constraints require clarification. No exact Planned TODO, BUGS, queue or ledger coverage was found. Proposed
drop: none.

Provenance: separate adversarial reviewer of commit 36 in the trailing review through
`738598601c142e9e184ee7f0427e8995b2d89e0f`; commissioner read the complete acceptance criteria and requirement sources.
