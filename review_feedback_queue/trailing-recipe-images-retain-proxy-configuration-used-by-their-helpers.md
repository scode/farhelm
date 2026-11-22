# Recipe images retain the proxy configuration used by their helpers

Reviewed commit: b23bbba44fa7bcf0d111696ceb744c54042e6eff

## TLDR

For ordinary image-backed login configuration, changing the place manager’s proxy or deleting one session does not
remove earlier credential-bearing settings from the shared recipe images.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/build.rs:165`.

build.rs:154–165 and :199–219 write the helper proxy file before software commit. Lines 349–379 repeat this before
preparation commit. podman.rs:644–673 installs it at mode 0600. The cleanup at tensorlake/build_script.rs:199–215 and
provisioning/tensorlake_backend.rs:207–300 has no step to clear it from the normal config directory before Podman commit
(podman.rs:529–580). The same ordering already exists in the parent of #4482 at build.rs:161, :209, :348 and :369. The
fresh-read change revises the session container file and does not rewrite the committed helper images. The normal
authority is the place's rootless Unix account and its Podman storage (SPEC_impl.md:6119–6121 and :6168–6174 at
7ceb053a0c34908b8f081ec48d2f31fd1b711a1a), with localhost-tagged images, mode-0600 file, and no host-account home or
socket bind in the environment (SPEC.md:5260–5265). The configured base route at the tip build.rs:837–870 refuses
prepared checkpoints. Its restriction is to prepared artifacts. Session Delete removes the container through
podman.rs:1439–1480, while recipe Delete requests removal of its owned images, keeping unconfirmed or in-use records
(tip build.rs:785–834). Retention is source-supported and pre-existing. A shipped ordinary path transferring or exposing
the retained bytes outside existing authority was not established and is the missing premise for a security consequence.
Bucket other. No runtime observation. Possible coverage none. Keep helper-only credentials out of captured layers while
retaining the separate session boot snapshot.

Independent check of the proposed exclusion. AMBIGUOUS via b23bbba4 and df619914:podman/build.rs:152–167,
podman.rs:527–556 and :642–671, tensorlake/build_script.rs:197–217, provisioning/tensorlake_backend.rs:205–234. Both
revisions install the same mode-0600 helper proxy file. The supplied excerpts omit the pre-refresh parent, the preparer
write and commit call sites, and the rest of snapshot cleanup. They cannot establish the full retention chronology or
rule out an additional retention instance. No additional recipient is established.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l5`.
