# The specification gives conflicting outcomes for a late sealed container Delete failure

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The product and implementation specs disagree about where a late sealed container deletion failure is reported.

## Details

480424a13df6824b256565bfd46fc678117e30f7 documents the lifetime of the Settings view's own failure display, which is in
that view's local state (crates/gui/farhelm-gui/src/settings/containers.rs:101–120, :313–339). After
7b747dcd8345a32b66bac92ea2eb1a8b2febc05d, a host-row deletion calls the seal finisher when the helm knows a seal under
the provisioning lock (crates/farhelm-helm/src/hosts.rs:1209–1211, :1244–1260). Both Host-level Delete and the Settings
leftover with a host row use that endpoint (containers.rs:684–692). crates/farhelm-helm/src/sessions/sandbox.rs:310–323
saves the latest reason even if the original reply timed out, crates/farhelm-helm/src/manager.rs:2840–2883 publishes it
in HostState::Deleting, and crates/gui/farhelm-gui/src/hosts/identity/phase.rs:432–437, :684 and hosts/remedies.rs:191
expose the reason and Retry in Hosts. 34a5995d52bd42912b193e38f2148d0148274653 added this known-seal exception to
SPEC.md:5206–5211 and to the sandbox rule at :1639–1644. It remains missing from the endpoint's SPEC.md:5257–5260.
Qualify that paragraph with the same exception and distinguish the closing Settings view from the helm's in-memory seal
diagnosis. An unsealed late failure still reaches the helm log only.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `host/f9`.
