# The new Update names can still coincide across an SSH host and a container

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

An SSH host and a container with the same display name can still have identical accessible Update labels.

## Details

5cd375fc5d3b7a4af4d4acf7c7db28181433482c replaced the uniform sidebar label with the full escaped name, which handles
coincident short SSH hostnames as intended by SPEC.md:282–293. crates/gui/farhelm-gui/src/hosts/update.rs:360–366 and
session_list/attention.rs:886–914 pass only that display name (plus the same busy reason if needed). The helm gives
unaliased SSH rows their literal destination and unaliased Podman rows the container name with the `podman:` prefix
removed (crates/farhelm-helm/src/aggregate.rs:118–138. Hosts.rs:320–326). Podman registration writes the prefixed
destination without an alias (store/podman.rs:381–389). The new-entry comparison uses raw destination equality and
compares the existing display name only against a proposed alias (store.rs:2218–2255). Destination-side collision
checking compares only other explicit aliases and the local default (store.rs:2263–2298), so a valid unaliased SSH
destination equal to an existing container's display name does not collide with its prefixed raw destination. Both types
can offer Update (store.rs:982–991 and crates/gui/farhelm-gui/src/hosts/offers.rs:545–554). With both update rows shown,
the rendered accessible names coincide. The Update action still carries its own stable host id. The adjacent identity
button can distinguish the full destination (session_list/attention.rs:822–854 and :944–953.
Session_list/host_rail.rs:297–312), so the ambiguity is specifically in tabbing or jumping between the Update buttons.
Use that existing identity text where it supplies a disambiguator. Add a disambiguator such as kind or destination when
full names coincide, preserving the escaped name in ready and busy states. The range has no later change to the label
paths.

Confidence: confirmed by inspection for two such independently registered, update-eligible hosts.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `hostUpdates/l6`.
