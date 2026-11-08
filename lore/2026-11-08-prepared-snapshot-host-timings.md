# Tensorlake hosts from one prepared snapshot

NOTE: Historical experiment recorded on 2026-11-08. These are two observations of a development build, not a latency
guarantee or a comparison with ordinary installation.

The prepared-snapshots plan moved Farhelm installation into a lazily renewed snapshot above a recipe's software
snapshot. This run created two ordinary hosts from that same prepared snapshot. Both remained connected together, with
distinct supervisor identities and distinct machine IDs. Each booted systemd, an active and persistently enabled
supervisor user unit, and lingering. Neither host ran the ordinary Farhelm installation steps.

The recipe used Tensorlake Ubuntu, one CPU, 1 GB memory and 10 GB disk, with no extra packages. The throwaway helm and
both hosts ran Farhelm `0.0.0-unreleased`, built from this plan's product code before its content-preserving rebase.
Payloads were staged from the matching glibc development build for Ubuntu; this did not validate a static-musl release
artifact. The credential selections were fictional Claude
Code and Codex endpoint credentials and a GitHub token. The selections differed between hosts without renewing the
prepared snapshot. Their private files and a real HTTP request through each host's configured tunnel were verified.

## Measured spawn phases

The helm measured these phases with one monotonic clock, starting after preparation and immediately before sandbox
creation. Admission includes waiting for SSH readiness. The final phase ends when the ordinary supervisor connection
is available. Each phase is floored independently to milliseconds, so their sum can be a few milliseconds below the
total.

| Host | Create to Running | Admission | Credentials | Attach | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| First | 899 ms | 974 ms | 18,614 ms | 1,167 ms | 21,655 ms |
| Second | 923 ms | 1,028 ms | 18,025 ms | 1,198 ms | 21,175 ms |

The first whole Add took about 109.1 seconds, including building and preparing the recipe. The second took about 23.6
seconds with that prepared snapshot reused. Those whole-operation observations include API polling overhead and are
separate from the phase timings. There was no ordinary-install baseline, no repeated sampling and no pass/fail target.
Credential delivery dominated both cached spawn observations; avoiding repeated installation does not make that work
disappear.

## Failure found and verification retained

An earlier attempt exposed a redundant linger action. Tensorlake preparation had already enabled lingering with
passwordless sudo, but ordinary installation repeated `loginctl enable-linger` without sudo. The image's masked polkit
service refused that repeat even though lingering was already enabled. Preparation now omits the redundant action and
still independently requires actual boot lingering, persistent unit enablement and verified state removal before
checkpointing. Ordinary host setup retains its existing behavior.

The successful recorder run was `9cf2b160-5387-4f04-b5b6-86f95e0c9ec8`. The failed linger attempt was
`8a7b9d37-8695-4357-8d63-1f450118f531`; an earlier harness startup failed before cloud creation because its state path
was too long for Unix sockets (`96e4ee56-3c10-4933-b111-47e0d4dfe984`). Private records retain the actual binary and
payload hashes and the failed observations.

Across the plan, six sandboxes were actually created, with eight conservative budget reservations including two unused
host reservations from the failed attempt. All six sandboxes, three snapshots and two registered test SSH keys were
confirmed removed. No images were created. The temporary helms' state was removed after evidence extraction; no live
installation was used.
