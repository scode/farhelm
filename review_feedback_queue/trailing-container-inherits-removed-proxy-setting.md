# New containers can inherit proxy settings the host removed

Reviewed commit: 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a

## TLDR

A new container session can still use a proxy removed from its host if the running systemd user manager retains the
earlier setting.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman.rs:667`.

The prepared image carries the helper proxy file in an ordinary image-backed login configuration (l5). Podman starts
systemd from that image (podman.rs:479–500). After the user manager has started, :1319–1361 writes the fresh file,
unsets keys and imports only currently present ones. provisioning/backend.rs:2990–3002 restarts the supervisor unit
without reloading the manager. Upstream systemd v255 manager.c:1763–1779 and :3549–3570 run environment generators at
startup and collect into transient_environment. dbus-manager.c:1784–1811 and manager.c:3745–3798 apply unsets to the
separate client list and merge it with transient values. unit.c:4954–4964 passes that result to new unit processes. A
present fresh key can override an old one, including with an explicit empty value. An absent fresh key has no override,
even though the file now omits it. On a subsequent container boot the generator uses the rewritten file at the active
path. Suspend/Resume of a paused container does not reboot its manager (podman.rs:689–765). The ordinary launch admits
after create (provisioning/tensorlake_add.rs:640–679), and supervisor service/core/host_launch.rs:112–169 and :503–526
preserve proxy variables in the inherited child environment by default. This conflicts with SPEC.md:5318–5320 and
SPEC_impl.md:6185–6189. Recompute the generated settings before restarting the supervisor, with test evidence
distinguishing an absent key from a present empty key. Bucket high for incorrect network routing/configuration on the
first launch. Confidence confirmed by pinned Farhelm and upstream v255 code for generator-backed systemd images with an
omitted old key. The exact distro package build and live outcomes remain unobserved. No disclosure is asserted. Possible
coverage none. Upstream https://github.com/systemd/systemd/blob/v255/src/core/manager.c#L3549-L3570
https://github.com/systemd/systemd/blob/v255/src/core/manager.c#L3745-L3798
https://github.com/systemd/systemd/blob/v255/src/core/dbus-manager.c#L1784-L1811
https://github.com/systemd/systemd/blob/v255/src/core/unit.c#L4954-L4964.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l8`.
