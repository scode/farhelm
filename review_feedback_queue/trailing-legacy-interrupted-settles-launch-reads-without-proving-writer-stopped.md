# Legacy Interrupted settles launch reads without proving the writer stopped

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

After same-boot legacy retirement, the ticker can stop checking for a later launch failure without proving the writer
ended.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/status.rs:676`.

#4529 distinguishes reboot and legacy retirement (SPEC_impl.md:3953–3956 and proto/src/lib.rs:865–868).
SPEC_impl.md:4125–4133 requires owned terminal death or boot-end proof to settle negative reads. In the legacy route, an
end failure returns no discovered roots and may leave the server in place (core/old_terminals.rs:255–279). The
subsequent agent sweep logs lack of confirmation and returns no proof to its caller (291–327).
core.rs:7110–7114,7142–7155 still commits the conversion and cleans artifacts. The later observer equates any
Interrupted with boot proof (status.rs:605–611,672–681), and ticker.rs:1198–1220 latches it after a negative read.
Cleanup of on-disk artifacts is not proof about a shim already holding its spec: launch.rs:1130–1153 consumes the file
before exec, and 1230–1237,2401–2411 records a later failure independently. Existing ticker.rs:3267–3295 asserts only
the known-boot case. Use actual boot finality or separately recorded retirement proof. Bucket: other. Confidence: source
confirms the missing-proof path. Remaining premise for an actual lost failure is a compatible historical shim surviving
the cleanup and later publishing the same generation's sentinel. This pass did not establish that concrete
survival-and-publication event. No runtime observation. for the proof mismatch. Possible coverage: l7 concerns the
different recovery-card promise.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l24`.
