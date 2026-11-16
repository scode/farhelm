---
kind: breaking
---

`farhelm helm setup`, which installed systemd user services running a helm and a supervisor on a Linux machine, is
gone. When this machine's supervisor is not running, the app now tells you to quit Farhelm and open it again, which
starts it, instead of pointing you at `farhelm helm setup`.

If you set up a Linux machine with `farhelm helm setup`, its services stay installed and keep running the `farhelm`
they were set up with (usually `~/.local/bin/farhelm`), whether or not you uninstall Farhelm; `farhelm uninstall` no
longer stops or removes them, so after an uninstall they fail on their next start. Those services are
`farhelm-helm.service` and, unless you set up with `--no-supervisor`, `farhelm-supervisor.service`, in
`~/.config/systemd/user` (or `$XDG_CONFIG_HOME/systemd/user`); each file that setup wrote starts with
`# managed-by: farhelm helm setup`. To remove one, run `systemctl --user disable --now` with its name and delete its
file.

Adding a remote host whose supervisor service `farhelm helm setup` installed, while that service is stopped, or
updating such a host, is no longer refused: the hosts panel replaces that service with its own. (Curator: uninstalling
such a host from the panel is still refused when the service runs a `farhelm` outside `~/.local/lib/farhelm`, as the
installer's `~/.local/bin/farhelm` is, now for that reason rather than for setup's. The panel never touches a remote
`farhelm-helm.service`, which setup also installed there; the cleanup above applies to it on that host. Setup also left
a harmless `.farhelm-setup.lock` file in the unit directory, which can be deleted with the services. The
`gpui-desktop.md` sentence that a setup-installed Linux helm keeps running stays true of services already installed.)
