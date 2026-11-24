# old_sessions_instructions references a nonexistent 'status unavailable' response from terminal_hosts / remote_terminal_hosts

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

The brick-test prompt for the `old-sessions` checkpoint tells the guest tester that if `terminal_hosts` or
`remote_terminal_hosts` returns `status unavailable`, it should report `could-not-run` with its detail. Neither
operation ever returns `status: unavailable`: instead, an unsupported host/child count trips the fatal action gate
(`managed-failure.json`), causing the coordinator to record the prerequisite mismatch as a guest automation failure.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/guest_workflow.py:146`.

In `releasing/mac-vm-test/guest_workflow.py:143-147` (`old_sessions_instructions`, edited in
`5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`), the managed prompt states:
`Then call terminal_hosts and remote_terminal_hosts: they record each session's terminal host and child process identities. If either returns status unavailable, report could-not-run with its detail.`
However, in `releasing/mac-vm-test/guest_operations.py:872-897` (`Operations.terminal_hosts`) and `443-484`
(`Operations.dispatch`), no code path returns `status: 'unavailable'`: calling before observing a captured session
raises `ObservationNotReady` (which returns `{'status': 'not-ready', ...}`), and observing
`len(hosts) != 1 or len(hosts[0]['children']) != 1` raises
`RuntimeError('old-sessions needs exactly one terminal host, with one live child, for its old session')`, which calls
`self.fail(...)` (writing `managed-failure.json` and closing the action gate) and emits an MCP `isError: True` response
(`OPERATIONS.md:232-234` also specifies that anything else closes the gate). Because `managed-failure.json` makes
`guest_tester.py:288-289` raise `TesterError`, the attempt always ends in `automation-failure` rather than letting the
tester return a clean `could-not-run` (though `brick_test.py:1039-1057` subsequently downgrades P1 `automation-failure`
to `could-not-run` with a `'Guest automation failed at this P1 check: ...'` label).

How to verify: Inspect `guest_operations.py:443-484, 872-897` and grep `releasing/mac-vm-test/guest_operations.py` for
`'unavailable'`: no operation returns a structured `'status': 'unavailable'` response.

Fix: Align the prompt and implementation — either remove the nonexistent
`If either returns status unavailable, report could-not-run with its detail.` sentence from `guest_workflow.py:146-147`
(since `OPERATIONS.md:232-234` and `test_guest_operations.py:1311-1350` explicitly require closing the gate), or
introduce an explicit structured unavailable response in `guest_operations.py` if clean non-fatal `could-not-run`
reporting without an automation-failure label is intended.

Reviewer metadata (g04b_brick_test_guest_workflow/f2): suggested bucket: other (documentation/prompt mismatch in
maintainer brick-test tooling; P1 still downgrades the resulting automation-failure to `could-not-run` so P0 is
unaffected).; confidence: confirmed (verified directly in `guest_workflow.py:143-147`,
`guest_operations.py:443-484, 872-897`, `OPERATIONS.md:232-234`, and `brick_test.py:1039-1057`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g04b_brick_test_guest_workflow/f2`.
