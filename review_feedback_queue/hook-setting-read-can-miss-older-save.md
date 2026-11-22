# Reopened hook Settings can miss an accepted save from the previous connection

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Settings reopened after reconnect can read the old hook switch before an accepted save from the previous connection
finishes.

## Details

**other.** Investigated uncertainty.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:5491-5513,5533-5557; crates/farhelm-helm/src/middleware.rs:39-61;
crates/farhelm-helm/src/lib.rs:1676-1694; crates/farhelm-helm/src/ui_channel.rs:239-332,806-827,998-1057;
crates/farhelm-helm/src/agent_hook_setup.rs:52-90,107-126; crates/farhelm-helm/src/store.rs:5606-5612;
crates/farhelm-supervisor/src/db.rs:190-199,229-265; crates/gui/farhelm-gui/src/session.rs:198-247;
crates/gui/farhelm-gui/src/settings.rs:230-236,1016-1029,1175-1207,1575-1593

Reset closes the old Settings dialog. An accepted old settings write can still be waiting in the helm while a reconnect
opens a new channel and Settings reads the stored switch. The two operations share a database mutex without a
cross-channel acceptance-order fence. If the read acquires it before the old write, the new dialog retains the prior
value because this switch does not follow the preferences feed. Reconcile the dedicated setting on subsequent committed
preference changes or order fresh reads after surviving writes. See
`crates/farhelm-helm/src/ui_channel.rs:239–332,806–827,998–1057`, `agent_hook_setup.rs:38–90,107–126`,
`crates/gui/farhelm-gui/src/session.rs:198–247,536–548`, and `settings.rs:1016–1029,1175–1207,1575–1593` at the pinned
tip. Practical incidence is unmeasured.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-helm/HH-02`.
