# Security notes

NOTE: This is not a security review and not a threat model for the whole system. It documents what the code does at
specific trust boundaries, and the tradeoffs that were taken, so that a later review has something concrete to check
against. It covers only what it covers; a boundary not described here has not been written up yet, which says nothing
about whether it is sound. SPEC.md's Security section is the product contract; SPEC_impl.md's auth paragraph under Helm
internals is the design summary.

## The desktop app's connection to its helm

The browser interface, its token exchange and its stored device credentials are gone, and with them most of what this
file used to say. What is left is the desktop app's window talking to its embedded helm over loopback with two
credentials minted in memory for that launch (SPEC_impl.md has the mechanics). One residual risk remains, and it was
accepted on purpose. (A second, script in the old web view reading the window's credential from page memory, went with
that web view: the GPUI window runs no script.)

### Another local user on the helm's port

The credential lets the helm tell its own window apart from other local processes and users. Nothing lets the window
tell its helm apart from another process on the same port. If the helm were down and another local user bound its port,
the window would send its requests, credential included, to them. No client-side mechanism closes that: a key the window
holds, a challenge it answers or an identity it checks is all checked by code talking to whoever answered. The one
complete answer is a TLS certificate chained to a trust anchor the client already holds, which for a loopback address
means the user installing a Farhelm-minted CA. Farhelm does not ship that.

The gap is accepted because it barely exists for the desktop app. Its embedded helm holds the port for the app's whole
life, picks a fresh port at every launch, and the app exits when its helm ends, so no window is left talking to a port
the helm gave up. The credential an impostor could catch is also worth little outside that launch: it lives only in the
helm's memory as a digest and dies with the process. The preconditions are another local account on the machine and a
helm that went away while its window stayed, which the app's lifecycle does not allow.

A related case does not need the helm to be down. The helm binds only IPv4 loopback (`127.0.0.1`), so another local
account can bind `[::1]` on the same port while the helm runs. The helm refuses `localhost` and `[::1]` as `Host` and
`Origin`, so no client of it works under a name that could resolve to that other listener.
