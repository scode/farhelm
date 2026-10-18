# A tunnel can take a port needed by a Mac service

Reviewed commit: 18de95db725464009717227c0130318d5ff43cc4

## TLDR

A consented localhost tunnel can occupy the remote URL's port on the Mac. A later local program expecting that port may
send traffic to the remote host, but the consent warning does not explain this possibility.

## Details

The helm prefers the remote URL's exact port when it can bind it (`crates/farhelm-helm/src/tunnels.rs:226–231,365–369`).
The warning describes cookies and local access but omits what happens to a later Mac service that expects the same port
(`crates/farhelm-ui/src/tunnels.rs:365–369`). Port collision is enough to misroute later traffic if a client still
connects to that address. A credential disclosure through an authentication callback is possible only if the callback
client sends it despite its intended service failing to bind; that premise has not been verified.

Suggested bucket: provisionally highest, for the possible credential consequence. Confidence: likely for traffic
misrouting conditional on a port collision; credential impact is unverified. Provenance: trailing review of the stated
commit. `SPEC.md` chooses the same-port preference and lists warning content, but does not accept omitting this
consequence. The landing report in `plans/reports/tunnel-localhost-links.report.md` also raises the warning gap. The
warning could name the port and explain that a later local service may not get it.
