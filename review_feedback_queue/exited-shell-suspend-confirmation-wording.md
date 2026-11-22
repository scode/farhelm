# The ended-shell Stop wording fix leaves its third answer inconsistent

Reviewed commit: 23ba5ede59ebae6523ad7f7bfd35f4ff7dc74352

## TLDR

Stop and suspend can still tell users it will stop a shell that has already exited.

## Details

**other.** Source inspection.

23ba5ede59ebae6523ad7f7bfd35f4ff7dc74352 crates/gui/farhelm-gui/src/session_list/stop.rs:96-105,130-149,170-181 and
environment.rs:555-588; 7b2b0512f69d17f5a64e3ca44a1842f8d7c7d152
crates/gui/farhelm-gui/src/session_list/environment_tests.rs:571-590,741-817

The patched ordinary Stop question correctly says it will close the remaining tabs of an ended shell. Its Stop and
suspend hover help still says it stops the shell, and the appended copy refers to the stopped shell. #4481 changes
Cancel and Confirm stop help only and the new tab-only test checks just those two answers and the opening clause. #4486
improves running-shell wording and guard coverage. Its direct Stop and suspend case starts from a running shell and
intentionally checks that wording stays captured after a push reports an exit, so it cannot cover the already-ended
click. The third answer sends the same frozen guard as Confirm stop (stop.rs:158-161,176-179; environment.rs:580-620);
the pinned helm passes the guard and withholds Suspend after a refusal or incomplete cleanup
(crates/farhelm-helm/src/sessions/sandbox.rs:559-591), and the pinned supervisor rejects an unexpectedly live or
uncertain main process before signalling anything (crates/farhelm-supervisor/src/service/handlers.rs:838-887). This is a
wording defect left over from the older prompt, not a new destructive authorization issue. SPEC.md at
1a53d9d35889d7dde96e31320cc1681dd013737d:1683-1689,2061-2066,2194-2211. The reviewer suggested other; confidence
confirmed; ; Possible coverage: original tab-only Stop help.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `session-prompts/SP-02`.
