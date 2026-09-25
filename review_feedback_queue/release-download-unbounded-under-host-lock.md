# Release downloads have no overall deadline under the host lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When the release download is extremely slow or throttled, the hosts being provisioned cannot be removed or edited until
the download finishes or the helm restarts.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F30 / SEC-DOWNLOAD-NO-DEADLINE-UNDER-LOCK`, tagged **possible**. Anchors and title: `provisioning/service.rs:1161`,
`provisioning/payloads.rs:780`, `provisioning/release_payloads.rs:549` — Release payload downloads have no overall
deadline and run while the host write lock is held

Downloading release payloads (`prepare_payloads`, called at service.rs:1161) happens inside the run task, which already
holds that host's `host_write_lock` and a fleet run slot (see F2). The HTTP client has a 15 s connect timeout and a 60 s
timeout per read, and — by explicit design (the client builder's doc at payloads.rs:756-760, builder at 777-780;
release_payloads.rs:549) — no overall deadline, so that slow links can finish. A slow server, or someone on the network
path throttling the TLS stream, can deliver a byte just often enough to beat each 60 s read timeout and keep the
download going indefinitely. The cached checksum list (a process-wide `OnceCell`) and the per-asset locks are shared
across runs, so other runs that need the same asset queue behind it.

The signature check means the bytes themselves stay safe; this is an availability problem only. Because the host lock is
held, every host waiting on such a download cannot be removed, retargeted or renamed (the lock coupling described in
F2). Suggested change: stage payloads before the run takes `host_write_lock` (or let removal cancel the run), and
consider a generous minimum throughput or an absolute ceiling, in the spirit of SPEC.md "Provisioning download sanity
limit".

User-visible consequence: when the release download is extremely slow or throttled, the hosts being provisioned cannot
be removed or edited until the download finishes or the helm restarts.
