# Removing a host strands its individually closable tunnels

Reviewed commit: 2b2216eaf95b6f2f514dff3f8beb3a90ae55930b

## TLDR

After removing a host, its tunnels can keep local ports occupied while their individual close requests fail. Disabling
all tunnels in Settings or quitting Farhelm releases them.

## Details

`crates/farhelm-helm/src/tunnels.rs:509–512` resolves the full session reference before inspecting its retained
listener. `sessions.rs:747–761` requires the entry UUID still to exist in the host registry, so removing the entry makes
that old reference return NotFound before local cleanup. Host removal (`hosts.rs:1039–1045`,
`provisioning/service.rs:1607–1624`) deletes the entry and stops its actor without removing tunnel listeners. Routing
failures do not end the listener's accept loop (`tunnels.rs:238–269`). Previously the local close needed only a match
against the retained session and tunnel ID.

Close the removed entry's listeners during removal, or permit local cleanup against retained entry/session identity
without requiring current registry membership. Preserve the boundary between different entries. A focused regression
would open an owned fixture tunnel, remove its host entry, and show that its listener can be released without revoking
every other tunnel. No live installation is needed.

Suggested bucket: **other**; resource cleanup and port availability, with no demonstrated security or user-work loss.
Confidence: **confirmed static control flow**, not runtime reproduced. Native UI behavior after removal was not
exercised. Settings revocation still closes everything (`preferences.rs:221–229`), so recovery remains available.

Possible cover: none exact. SPEC.md's tunnel lifetime and local-close contract and SPEC_impl.md's listener ownership
rules do not accept stranded listeners after host removal. Planned TODO, BUGS, queue and ledger searches found no
matching trigger, consequence and scope. This persists beyond reconnect or view closure and does not match the rare
self-correcting-glitch filter. Proposed drop: none.

Provenance: separate adversarial reviewer of commit 73 in the trailing review through
`738598601c142e9e184ee7f0427e8995b2d89e0f`; commissioner inspected the routing and host-removal paths. No runtime test
or historical product-suite result is claimed as current evidence.
