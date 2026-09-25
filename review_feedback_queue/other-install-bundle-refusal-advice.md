# Uninstall advice for another install's bundle hijacks it

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With two Farhelm installs on one Mac, uninstalling one is refused with advice that, if followed, hijacks the other
install's app bundle instead of fixing the refusal.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F11 / COR-OTHER-INSTALL-BUNDLE-ADVICE`, tagged **possible**. Anchors and title:
`crates/farhelm/src/uninstall/ownership.rs:517` — When the app bundle belongs to another install, the refusal advises
rerunning the installer, which would take the bundle from that install

A Mac can hold two Farhelm installs in different directories (the default `~/.local/bin` and a custom
`FARHELM_INSTALL_DIR`), but there is only one `~/Applications/Farhelm.app`; its receipt names the install directory that
built it. When uninstalling install A while the bundle's receipt names install B, `inspect_bundle_at` refuses at L517
through the shared `repair_refusal` helper, whose advice is fixed: "rerun the installer, or upgrade once to a release
that writes uninstall metadata". The metadata is valid; it just belongs to B. Following the advice (rerunning the
installer for A) rebuilds the bundle from A's binaries, silently taking the app away from B and making B's own uninstall
refuse with the same message later.

SPEC.md requires refusals about unclear ownership to say what to do; here the only suggested action damages the other
install without addressing the real situation. Suggested change: give this case its own message that names the recorded
origin directory and says to uninstall from that install's CLI first.

Restater note: the finding also says the same advice appears when the recorded origin directory no longer exists
(`field_path` fails to canonicalize it and says "rerun the installer to repair metadata"). In that sub-case there is no
other live install to take the bundle from, so rerunning the installer for the current install is harmless and does
resolve the refusal; only the two-live-installs case is damaging.
