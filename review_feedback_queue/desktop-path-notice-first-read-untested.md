# Startup tests do not see whether the first client gets the PATH notice

Reviewed commit: 1d4fd8a723e5a195f61e625ecf62e8c3a347eb29

## TLDR

The startup tests can pass if the first client misses the login PATH warning.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed source-only test gap: the desktop smoke-style helper call hard-codes true (desktop lib.rs:1866). The lifecycle
test directly sets `desktop_login_path_available` and opens the database only after awaiting the server function to
finish (helm lib.rs:1964–1994). It can pass if the write is moved after readiness, provided it still completes before
shutdown. The new GUI test supplies a prebuilt pushed entry (rail_notices/tests.rs:371–400) and the shared real-helm and
e2e harnesses both supply `None` (real_helm.rs:52, harness.rs:865). A source search of the lookup and notice symbols
found no existing test crossing desktop result, embedded handoff and first client read. The last state assertion also
models Clear resolved through raw SQL (helm lib.rs:2029–2045). The actual shared mark route has separate coverage, so
this does not establish another behavior defect. SPEC_impl.md:3922–3925 explicitly requires the handoff and
reconciliation before serving, and :8303–8307 requires the pushed rail display. Bucket other: regression coverage, with
no current production failure asserted. Possible_cover: none in the governing spec. No tests executed. The later
5f97d10518d6b2e598ecab8e8387e0f78f6e68d8 desktop test checks how a relaunch command inherits PATH. It adds no coverage
for the first client read or failed lookup handoff. No later in-range fix through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `path-notices/f7`.
