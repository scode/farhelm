# Package repair does not wait for dpkg lock

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

The repair step does not share the package installer's lock-wait policy and could fail under contention. A supported
concurrent package-manager trigger remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/build_script.rs:92-95 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g069/g069-l2, event 15):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c; unchanged assigned file through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: If Ubuntu has another package operation running at this point, snapshot
creation can fail before Farhelm's advertised package-lock wait helps, requiring retry of a billed build.
Source-confirmed fact: build_script.rs:92 puts DPkg::Lock::Timeout=600 solely on apt-get invocations in apt_get(); line
93 first invokes direct sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a with neither an explicit
lock-wait bound nor a retry, and immediately calls fail on a nonzero result. lines 94-95 reach apt_get only afterwards.
ready() accepts systemd running/degraded and passwordless sudo; it does not acquire package locks. build.rs:734-755
sequences that check then the package script, and :1101-1112 propagates a failed script instead of retrying it. The
installed primary dpkg packaging document /usr/share/doc/dpkg/spec/frontend-api.txt, Database Locking, describes
separate frontend and dpkg database record locks and states dpkg acquires the database lock itself. This confirms why
the direct repair step participates in package-manager locking; it does not establish the Tensorlake image's boot
timing. The code comment at build_script.rs:81-82 says Ubuntu's daily timers can own the lock just after boot, but that
statement is rationale, not an observed fixture proving contention at this step. Synthesis checks: searched pinned
Tensorlake source, docs, packaging, website/src and specs for base-image, image name, apt-daily/unattended-upgrades and
machine-ID references; read build.rs:76-79, SPEC.md snapshot-build section and the Tensorlake guide's recipe/build
documentation. They name external tensorlake/ubuntu-systemd and require systemd/passwordless sudo, but provide no
Dockerfile, timer package/version/enabled-state declaration or boot-event trace. Local dpkg docs describe generic
packaging behavior, not that external image. Therefore actual occurrence remains unverified after investigation: missing
premise is a competing package operation holding a lock when this builder's direct dpkg repair runs. A provider image
definition or retained narrow lock-contention evidence could settle it; no experiment or external-image inspection was
performed. Suggested fix conditional on confirming the supported image route: make the repair participate in a bounded
lock wait/retry, without deleting lock files or killing their holders. Suggested bucket other; confidence likely for the
conditional failure, not confirmed reproduction on the shipped image. No established user-work/security loss, and old
snapshot preservation is unchanged. proposed_drop false; possible_cover unknown (forbidden coverage sources not
searched). This synthesis adds evidence boundaries, not a new discovery, and preserves the earlier finding history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g069/g069-l2`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
