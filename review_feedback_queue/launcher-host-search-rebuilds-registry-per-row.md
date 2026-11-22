# The wide host search rebuilds the host list for each matching row

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Wide host search rebuilds the full host list for each matched row, increasing work as the fleet grows.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search/start.rs:539–564;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view/search/start.rs:568–594;
crates/gui/farhelm-gui/src/launcher/view/search.rs:914–941; crates/gui/farhelm-gui/src/launcher/search.rs:544–551,
567–583; crates/farhelm-helm-client/src/collections.rs:335–354

Wide host search spends more work per repaint as the host list grows. Every matching row calls ordered<HostView>(),
which materializes the whole registry vector before the row checks its own connection status. An empty host filter can
include every selectable host, making registry work quadratic in the number of hosts on that redraw. The result was
already constructed from the HostView and could retain its connected bit, or rendering could use a keyed lookup. The
reviewer suggested other given the documented assumption of few hosts (SPEC_impl.md:5507). Confidence confirmed for the
repeated work and allocation. User-perceived slowdown in a fleet was not verified. ; Possible coverage: LO-08 covers a
separate per-template collection lookup and launcher-hosts-quadratic-underlining covers text matching.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-search/LS-13`.
