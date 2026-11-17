---
kind: breaking
---

`farhelm helm run` is gone: the helm now runs only inside the Farhelm desktop app, which starts its own. Its release
mirror setting, `--release-base-url` or `FARHELM_RELEASE_BASE_URL`, went with it, so the helm always downloads the
software it installs on hosts from Farhelm's own releases.

If you set up a Linux machine with `farhelm helm setup`, its `farhelm-helm.service` runs `farhelm helm run` with the
`farhelm` it was set up with (usually `~/.local/bin/farhelm`). Once that `farhelm` is replaced with this release or a
later one, the service fails to start. To remove it, run `systemctl --user disable --now farhelm-helm.service` and
delete `farhelm-helm.service` from `~/.config/systemd/user` (or `$XDG_CONFIG_HOME/systemd/user`); `farhelm uninstall`
no longer does this for you.

(Curator: the other `helm run` options went too: `--port`, `--state-dir`, `--ensure-hosts` and `--payload-dir`. For
developers, a desktop app built from a working copy still reads its payload directory from `FARHELM_HELM_PAYLOAD_DIR`;
an installed app never sees that variable, so it is not worth an entry. The unit name, its directory and the
`helm run` command line it runs were checked against the code `farhelm helm setup` had before its removal
(`crates/farhelm-helm/src/units.rs` and `units/farhelm-helm.service.in` at 0de00c7c4^). Neither the installer, which
refuses Linux, nor the hosts panel, which installs under `~/.local/lib/farhelm`, replaces `~/.local/bin/farhelm` on
Linux today, so the service breaks when someone replaces that file by hand. That is currently the only way a Linux
machine gets a newer `farhelm` (the installer refuses Linux and no Linux desktop archive ships), so every Linux setup
user who moves to this release meets the failure: keep the caveat. This sits next to the
`farhelm helm setup` entry from `remove-helm-setup.md`, whose "keep running" sentence was edited to point here.)
