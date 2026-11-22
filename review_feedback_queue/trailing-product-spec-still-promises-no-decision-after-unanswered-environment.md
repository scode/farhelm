# Product spec still promises no decision after an unanswered environment Delete

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The product spec groups a request that was never sent with one that went unanswered, even though an unanswered Delete
may proceed.

## Details

**other.** Source inspection. Main reference `SPEC.md:1649`.

#4529 repairs this exact contract in SPEC_impl.md:6224–6229 and 6258–6262 by bounding admission and saying a sent
decision can remain pending or have committed. SPEC.md:1648–1653 still says ‘nothing is decided’ for both cases, despite
1636–1639 documenting lost-reply seals and 1661–1667 documenting the route deadline. The supervisor starts an owned
mutation, times only admission (handlers.rs:1365–1406), then performs preflight and can commit (1413–1474). Shipped helm
sessions/sandbox.rs:228–233 uses definite wording with no client; 245–263 reports uncertainty for timeout or early lost
connection, and 440–446 says a pending Delete may still complete. Its comments at 124–133 and 155–156 keep the obsolete
absolute. The GPUI confirms the frozen route (delete.rs:270–313). An early marked refusal constructs a fresh
whole-environment question (environment_delete.rs:303–320), while an outer-bound 504 takes the earlier carries-on
branch. Existing environment_delete_tests.rs:483–518 asserts no second request before a separate affirmative answer, and
593–614 checks carry-on handling. Reconcile wording and comments with the three outcomes. Bucket: other, documentation
only. Confidence: confirmed by pinned source and existing assertions, with no runtime observation or consent violation
found on this path. Possible coverage: l18 for the same spec paragraph, distinct claim.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l1`.
