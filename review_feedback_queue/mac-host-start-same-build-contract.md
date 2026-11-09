# Remote Mac update contradicts the same-build startup invariant

Reviewed commit: 25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652

## TLDR

The Mac update design lets an old supervisor start a terminal host from the replacement build, while the terminal-host
contract says starter and host always have the same build. This is a confirmed specification inconsistency; no
incompatible startup data or actual session-start failure was demonstrated.

## Details

The changed `SPEC_impl.md:1895` says Mac host startup uses the supervisor's program path, versioned only for the desktop
app. Remote Mac Update replaces the flat executable before refreshing launchd
(`crates/farhelm-helm/src/provisioning/plan.rs:774–804`, ordered execution in `service.rs:2489–2504`). A still-running
old supervisor can start a terminal in that interval.

Production `crates/farhelm-supervisor/src/service/core/host_launch.rs:379` selects `own_host_program`.
`hosts/spawn.rs:195–199` uses the Mac pathname, rather than Linux's running-inode path; lines 349–360 execute it with
the caller's encoded child specification. Lines 508–530 encode the startup frame using the running supervisor build. The
replacement program and old encoder can therefore differ.

`crates/terminal-host/SPEC.md`, "Change policy", at lines 525–529 excludes command-line and child-spec startup data from
cross-version surfaces because both sides "always come from the same build". Stable wire/tag/snapshot contracts at lines
501–521 do not establish that separate startup premise. The startup fields are kept stable, and no child-spec
implementation changed in this reviewed range; those facts limit the demonstrated consequence without making the
same-build claim true. No concrete incompatible historical/current pair was identified.

Either preserve executable build identity for remote Mac starts during Update or document the cross-build startup
exception and its compatibility obligation. The later upgrade-floor policy limits supported update sources but still
permits different builds above the floor, so it does not restore this invariant.

Priority: `other`. Confidence: confirmed source/invariant contradiction only; runtime impact unestablished. Proposed
drop: none. Possible cover: existing stable startup fields limit consequences, not the stated invariant. No exact
accepted exception, planned fix, known bug, queue item, ledger decision or filter match was found. The separate Linux
launch-shim exception does not cover this Mac host-spawn boundary.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `mac-impl-contract/mac-impl-contract-discovery-executable`. The changed
contract came from `29db8696aca21938bb91f35d03d9951f2f1dff0a`; unchanged terminal-host policy was governing context.
