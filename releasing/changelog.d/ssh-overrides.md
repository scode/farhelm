---
kind: fixed
---

Farhelm's ssh connections to your hosts no longer forward your ssh agent, X11 display or ports, even when your ssh
config turns forwarding on (for example `ForwardAgent yes` under `Host *`). The connection to a host's supervisor stays
open for as long as the helm runs, so such a setting used to give anything running on that host use of your ssh keys the
whole time. A host whose ssh config entry sets `RemoteCommand`, or a `LocalCommand` with `PermitLocalCommand yes`, can
now be added, set up and connected to; before, ssh refused Farhelm's commands or mixed the local command's output into
Farhelm's connection. Your ssh config still decides how a host is reached and how you authenticate (keys, agent,
ProxyJump, Match blocks). One exception right after upgrading: if the new version starts within a minute of the old one
stopping (as when you quit the desktop app, install the update and reopen it, or restart a helm you run by hand), ssh
can reuse the old version's shared connection, and any port forwards your config set up on it stay until that connection
closes. Stopping the helm for over a minute closes it.

For curation: Farhelm now needs OpenSSH 7.6 or later on the helm's machine.
