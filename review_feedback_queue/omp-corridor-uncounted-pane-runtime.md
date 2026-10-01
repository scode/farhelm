# An unreadable foreground OMP command line lets a nested OMP take over the session's conversation

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Sometimes Farhelm cannot recognize the OMP program running in a session's terminal. The main trigger is a launch command
line longer than 64 KiB, for example a large inline system prompt in a profile. In that case, an OMP started underneath
it can report its own conversation as the session's. Farhelm records it as the session's resume target, and a later
Restart resumes a conversation the user never had in that session.

## Details

Source: gap-filling review pass, 2026-09-30, slice sup-store2.

Reviewer's confidence: likely. The code path is traced end to end. Unverified premise: that a real OMP child carrying
the gated extension and the session's inherited credential can run directly under the foreground runtime. SPEC.md and
the admission tests assume this "separately launched interactive child" exists; I did not confirm which OMP feature
produces it..

Reviewer's bucket suggestion: highest.

Possible cover for triage to check: none. SPEC.md ~1089-1092 requires the opposite ("not a nested OMP process that
inherited its credential").

- **The pane process is the runtime.** For an ordinary OMP launch, the process tmux reports for the pane is the OMP
  runtime itself. The login shell `exec`s the launch shim, which `exec`s the agent
  (`crates/farhelm-supervisor/src/launch.rs` ~566-600, `window_command`; `exec_launch_spec` at 815/898).
- **Bun runtimes are only recognized from their command line.** `is_omp_runtime_link`
  (`crates/farhelm-supervisor/src/procs/omp.rs:416-428`) counts a Bun image as an OMP runtime only when its argv is
  present and its entry point matches or resolves to OMP's bundle.
- **That command line is dropped above 64 KiB.** `read_process_argv` returns `None` for any process whose command line
  exceeds `MAX_ARGV_BYTES_PER_PROCESS` (64 KiB, `procs.rs:199`) or cannot be read.
- **An unrecognized pane is never checked.** `omp_corridor` (`procs/omp.rs:282-393`) looks for exactly one runtime link
  (321-332). The pane anchor, the last link, is then skipped by position in the intermediary check
  (`index + 1 == chain.len()` at 348). So for the chain
  `[hook, nested OMP runtime, foreground Bun pane with argv None]`:
  - the foreground pane is not counted as a runtime;
  - the nested runtime becomes the only "emitter";
  - nothing sits between them that needs classifying;
  - the corridor returns `Ok(nested)`.
- **The same chain is refused today only when the pane is recognized.** It is exactly the shape the regression test
  `a_nested_omp_runtime_is_refused` pins (`procs.rs:2455-2466`, `[hook, runtime 12, runtime 11]`). That test passes only
  because the pane link has usable argv.
- **What admission then does.** Admission (`service/core/vendor/omp.rs:120-165`) sees the session's provenance naming
  the current asset (the launch was injected, since injection looks only at the program name `omp`). The nested child
  passes the asset's interactive gate, as SPEC.md itself says. The corridor admits it, and
  `admit_ownership_proven_conversation` (`store.rs:5041-5076`) commits the child's `omp:` locator with
  `capture_ownership_version = 1`.
- **Nothing can override it.** The real foreground's own reports are refused ("no attributable OMP runtime"), because
  its argv is still unavailable. The child's conversation stays the durable Resume target.
- **A second, less certain trigger.** The pane's Bun argv is present but its entry spelling no longer resolves: a
  relative script path resolved against the process's current `/proc/<pid>/cwd` (`resolved_omp_entry`, 441-455), or an
  entry removed by an uninstall. I did not verify this one.

**How to verify:** add a `procs.rs` unit test with
`chain = [hook link, omp_runtime_link(12, &[]), ChainLink { pid: 11, exe: "/opt/bun/bin/bun", argv: None, .. }]` and
`OmpLaunchProgram::Omp`. Today `omp_corridor` returns `Ok(pid 12)`; it should refuse.

**Fix direction:** fail closed on links that might be an OMP runtime but cannot be classified. Any non-reporter link
other than the emitter, the pane anchor included, that is a Bun or Node image with missing or unresolvable argv, or that
is otherwise indistinguishable from a runtime, should refuse. Alternatively, for `OmpLaunchProgram::Omp`, where no
launcher is admitted, refuse when the pane anchor is a Bun image that is not the emitter. Codex and Grok recognize their
runtimes by image alone (`procs/codex.rs:46`, `procs/grok.rs:58`), so they are not affected.
