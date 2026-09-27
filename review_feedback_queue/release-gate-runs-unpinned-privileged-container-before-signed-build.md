# Release gate boots an unpinned privileged CentOS container on the runner that then builds the signed Linux payload

Reviewed commit: b529cfd641a6f02e4b035852e0ddf501f2068046

## TLDR

Every tagged release pulls whatever `quay.io/centos/centos:stream9` happens to be that day, runs it as a fully
privileged container with the runner's cgroup tree mounted read-write, and only afterwards compiles and packages the
Linux archive that provisioning pushes onto every fleet host and that the signing job then blesses. Nothing pins that
image by digest and nothing verifies it. A poisoned image (or a poisoned package the image's own repository keys accept)
would end up inside a correctly signed Farhelm release that every helm installs without complaint.

NOTE: the precondition is a compromise of the CentOS image tag or of the CentOS package-signing chain baked into it.
This is a hardening gap in the release's trust chain, not a bug an ordinary attacker can reach; triage should weigh it
as such.

## Details

Where it runs, and in what order. `.github/dist-build-setup.yml` is spliced into the top of every
`build-local-artifacts` job; its "Provision a CentOS Stream 9 host over ssh" step (lines 289-305, gated to
`runner.os == 'Linux' && runner.arch == 'X64'`) runs `scripts/test-provision-centos.sh`. In the generated
`.github/workflows/release.yml` that step lands at lines 239-246, while `Install dist` and `dist build`, the steps that
produce the archive, come later at lines 379-394 in the same job on the same runner. The container executes before the
bytes that get signed exist.

What the container is. `scripts/test-provision-centos.sh:214-215` writes a Dockerfile whose first line is
`FROM quay.io/centos/centos:stream9`, a mutable tag with no `@sha256:` digest, and installs `systemd` and
`openssh-server` with `dnf` inside it (line 218; dnf verifies packages against the GPG keys shipped in that image, so
the image tag is the entire trust root). Lines 259-266 start it with `docker run -d` plus `--privileged`,
`--cgroupns=host` and a read-write bind of `/sys/fs/cgroup`, running `/usr/sbin/init`. `--privileged` gives the
container's root every capability and every host device; with the host cgroup tree writable as well, escaping to the
runner VM is routine (mount the root block device, or the `release_agent` route). The runner user is in the docker
group, so this is root-equivalent on the machine holding the checkout, `~/.cargo`, the toolchain, and the `target/` tree
that `dist build` is about to consume.

Why the rest of the pipeline cannot catch it. `sign-sums.yml` reasons about exactly this class of risk for the release
CANDIDATE (lines 9-25: "a runner that has executed an unsigned candidate is a runner that candidate may have left
something on") and isolates the signing key on a fresh machine. That isolation protects the key, not the build.
`validate` checks archive shape, static linkage, architecture and that the binary serves its UI, all of which a trojaned
build passes, and `sign` only proves that the published bytes are the bytes `validate` looked at. Nothing compares the
built archive to an independent build. A compromised runner during `dist build` therefore yields a release with a valid
`SHA256SUMS.minisig`, and `release_payloads.rs`'s verification (the compiled-in `MINISIGN_PUBKEY` and the
`farhelm v{version}` trusted comment) accepts it, as it should.

Contrast with everything else the same job executes before the build, which is pinned by checksum: tmux, libevent and
ncurses (`.github/release/source-pins.env` plus `scripts/build-private-tmux.sh`), the zig wheel
(`ziglang-requirements.txt` with `--require-hashes`), nextest (`.github/nextest-pins.json`, archive and binary digests),
minisign (`sign-sums.yml`). The CentOS image is the one third-party executable input in the release path with no pin at
all. `dist-workspace.toml`'s "Residuals in the GENERATED workflow" header (lines 143-174) lists the three unpinned
things it knows about (tag interpolation, the dist installer script, workflow-scope permissions) and does not mention
this one, so nobody reading the documented residuals would learn of it.

To verify: `grep -n 'FROM\|docker run' scripts/test-provision-centos.sh` and compare step order in `release.yml` (CentOS
at 239, `dist build` at 394). `docker image inspect quay.io/centos/centos:stream9` on any machine shows the tag
resolving to a different digest over time.

Fix: pin the base image by digest in `source-pins.env` (something like `CENTOS_IMAGE_DIGEST=sha256:…`) and have the
Dockerfile use `FROM quay.io/centos/centos@${digest}`, with the same bump-and-verify note the other pins carry; the
`dnf install` layer is then anchored to keys inside a pinned image. Optionally also record the digests of the installed
rpms, or build the image once and store it as a checksummed release-gate input. Dropping `--privileged` is not realistic
for a systemd-as-PID-1 container, which is why the pin is the lever. Moving the leg to a separate job is constrained by
the fail-open dist graph documented in `dist-build-setup.yml:16-25`; if it is moved, the build job must `needs:` it
rather than the reverse. Whatever is chosen, add it to the residuals header in `dist-workspace.toml` or remove it from
the list of unknowns there.

Adjacent queue items: none. `installer-mirror-var-drops-https-no-signature.md` and the helm's own download verification
concern the consumer side of the same chain; this is the producer side.

Unverified: whether GitHub's docker daemon on `ubuntu-latest` applies any additional confinement to `--privileged`
containers. I believe it does not, and the script's own comments rely on systemd getting real cgroup control.
