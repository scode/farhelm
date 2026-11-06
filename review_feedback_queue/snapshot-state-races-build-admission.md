# Snapshot state races build admission

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Adding a host as a build finishes can select a just-deleted snapshot or start an unnecessary charged build.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/build.rs:661-717 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g070/l3, event 13):

Reviewed commit ca28b604b876770b1ab757783416f83ad1709313, unchanged through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b per initial history check. TLDR: adding a Tensorlake host as a build finishes
can either use a snapshot that was just deleted or start an unnecessary charged build. Consolidates former l4 into l3 as
one non-atomic acquisition mechanism with distinct consequences. Rechecked build.rs:661-675 reads state independently of
registry admission; start_definition:563-595 only checks locally active/deleting and never rechecks ready-snapshot
presence. Build completion records S2/retires S1 at 896-909, cleans at 913-924 and removes active at 625-637. With an
earlier S1 state read, waiting setup can see no active build and return deleted S1. With an earlier None state read, it
can see no active build and start B although A recorded a usable snapshot. Initial caller evidence remains
provisioning/tensorlake_add.rs:397-425 and 537-587: consumes the returned ID without retirement protection or
invalid-source re-resolution, then fails setup on refusal/terminated sandbox. SPEC_impl.md Adding Tensorlake
hosts:5749-5753 explicitly requires ready reuse/active join and freezes only a build the add starts. c1's
sender-lifetime refutation and l2's accepted join remain intact. Fix distinction: taking the registry observation before
the state read can narrow the finish-between-reads cases, but does not protect S1 between selection/return and create
consumption; another rebuild or recipe cleanup can retire it during later awaits. A complete correction must coordinate
reuse/join/start admission and retain an acquired snapshot until provider create has consumed it, or recover an
invalidated source safely for an uncreated host. Explicit Settings rebuild must still be unconditional; blindly changing
start_definition to always reuse would break that action. Suggested bucket other; source-level interleavings confirmed,
provider's exact missing-snapshot error shape not established, no existing user-owned work loss claimed. proposed_drop
false. possible_cover: l4 is merged here, g047-l1/g065-r3 share the absence of retirement protection but describe a
separate cleanup-versus-host-admission race. No runtime tests run and no new discovery claimed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g070/l3`; supporting records
`g068/s4`, `g047/interface-g070-l3`, `g065/g070-l3`, `g070/l4`, `g071/c9`. The retained private log preserves earlier
wording and its corrections. This record does not claim runtime reproduction.
