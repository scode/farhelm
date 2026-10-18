# Payload work can outlive the helm's state-directory lock

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An exceptional helm shutdown can release its state directory while old download or extraction work is still running. A
new helm can then use the same payload cache concurrently. The ownership gap is confirmed; corrupt installation or
execution of unauthenticated bytes has not been demonstrated.

## Details

`crates/farhelm-helm/src/lib.rs:2119–2130` returns from serving without draining provisioning's blocking work. Dropping
the control guard releases the flock (`token_control.rs:79–113`) before the enclosing runtime is destroyed
(`crates/farhelm-ui/src/desktop.rs:488–518`; `crates/farhelm/src/main.rs:1200–1205`). Already-started blocking closures
can finish during runtime teardown. They retain no state-directory ownership. Joining the old thread does not prevent a
separate process acquiring the released flock.

The payload cache assumes one writer, uses a process-local mutex
(`crates/farhelm-helm/src/provisioning/release_payloads.rs:402–414`), and uses fixed staging names (`:739–797`).
Housekeeping unlinks staging files and generations (`:1109,1123–1143`); publication uses rename (`:847–865`). Two
independently verified publications alone do not establish an authenticity bypass. Unlinking also isolates an
already-open old staging inode. Neither observation closes a late-open/truncate schedule in which old work opens a
replacement staging pathname, or continuing download work before asynchronous task teardown completes.

The download hashes incoming chunks rather than independently authenticating the final inode. Extraction and private
upload staging preserve the bytes supplied to them; copying and hashing those bytes is not a second check against the
signed archive. This leaves an unresolved integrity question under concurrent old/new writers. No complete schedule
installing attacker-controlled bytes was established.

Keep ownership alive until all work that can touch shared cache paths finishes, or give old work paths that a successor
cannot reuse. A focused reproduction should pause blocking work, force a serving return, start a second helm after lock
release, and inspect late opens and final authenticated publication. It must use isolated state.

Suggested bucket: provisionally **highest**, because the possible consequence affects payload authenticity. Confidence:
**confirmed lifetime gap; conditional and unverified damaging consequence**. Requires exceptional serving return or
unwind with active provisioning and a successor starting. Ordinary desktop Quit exits the process and is explicitly
excluded. Ordinary browser cancellation does not supply this trigger: accepted provisioning owns its task.

Possible cover: SPEC_impl.md's Who owns an accepted action permits process-ending interruption, but does not explicitly
accept surviving concurrent writers after ownership is released. SPEC.md's single-helm assumption and Healthy local
filesystems rule do not clearly cover a successor admitted by the lock. The ledger's
`shutdown-leaves-retained-supervisor-clients-active.md:12255–12266` concerns asynchronous clients, not surviving
blocking filesystem work. The rare-safe-failure filter would apply only if the whole consequence were established to be
safe retryable failure; that remains unresolved. No exact Planned, BUGS or queue coverage was found.

Proposed drop: rejected by an independent fresh-context Astra/high reviewer because the integrity uncertainty remains.
Provenance: `provisioning-release-integrity-1/f1`, `desktop-platform-1/f2`, and `helm-authentication-1/f3`; one
underlying ownership defect.
