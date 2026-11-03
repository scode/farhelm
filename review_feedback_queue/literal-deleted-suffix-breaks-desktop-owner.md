# A literal executable suffix breaks desktop ownership recognition

Reviewed commit: a98147fb2eaaefb448473107529ada69114d7b03

## TLDR

A developer using an executable whose name ends in ` (deleted)` can lose ownership recognition. Selecting another
development build can then reuse the earlier answering supervisor. Fixed-name installed programs are outside this route.

## Details

`crates/farhelm-supervisor/src/procs.rs:243–247` removes a trailing ` (deleted)` unconditionally, including on macOS.
That can be the executable's actual name rather than Linux procfs decoration. `ProgramFile` retains the real canonical
name, so `managed_supervisor.rs:239–241` classifies the observed process as another image. The desktop discards
ownership and can reuse an answering supervisor rather than replace it. Arbitrary development executable overrides are
accepted by `crates/farhelm-ui/src/desktop/bundle.rs:394–412`.

The retained sequence leaves the original executable linked, then selects another build path. It does not cover Linux
atomic replacement at the same path: procfs adds another suffix there, and removing one recovers the actual basename.
Correct normalization without stripping a literal filename component, preserving the platform distinction.

Suggested bucket: `other`. Confidence: source-confirmed conditional development-override mismatch; incidence was not
measured. No installed-program or user-work-loss defect is claimed. No exact coverage was found; persistent wrong-build
reuse does not fully match the self-correcting-glitch filter.

Provenance: g020/f4; Codex gpt-6.1-sol, high reasoning. The broader Linux same-path claim was corrected under the
original finding ID and is not retained here.
