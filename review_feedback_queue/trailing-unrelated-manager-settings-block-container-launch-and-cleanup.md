# Unrelated manager settings can block container launch and cleanup

Reviewed commit: 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a

## TLDR

A place or custom image can be refused even when its proxy and path settings are small and readable. A failed place
refresh can also prevent Farhelm from removing an existing container.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:738`.

podman_probe.rs:734–772 captures the whole Environment array or show-environment fallback before selecting 10 names.
podman.rs:614–638 captures the whole in-container manager block to select XDG_CONFIG_HOME. The capture ceiling is 64 KiB
per stream (provisioning/backend.rs:51, :1936–1945, :1967–2007). An overflow is a command failure. systemctl(1),
Environment Commands, permits a manager block bounded by the platform argument limit, which can exceed Farhelm's cap.
Ordinary provisioning already selects XDG_CONFIG_HOME before returning output for that reason (backend.rs:852–904).
Previously the full-block reads affected probes and container directory selection. The patch adds an uncached place read
before create admission (podman.rs:1339–1345), even while capability admission can reuse recent results
(podman_probe.rs:425–434). An ordinary failed refresh publishes over the previous success (:513–529). Runtime thereafter
refreshes until it receives the failed result (:382–419). Podman command resolution, including removal/absence checks,
depends on that runtime result (podman.rs:183–218, :239–270 and :447–470 at the tip), so it never reaches Podman while
the unrelated settings keep the refreshed probe failing. SPEC_impl.md:6185–6195 requires the selected proxy values and
cleanup and gives no full-manager-size requirement. Filter before bounded capture, preserving exact-value and redaction
semantics, and keep proxy acquisition out of cleanup-only command admission. Bucket other for configuration-specific
launch/cleanup failure. Confidence confirmed by source and documented interface. The live size, runtime outcome and
legal unsupported-escape theory were not observed, and the escape theory is not a premise. The cleanup diagnosis is not
the full Podman inspect overflow described by l12. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l11`.
