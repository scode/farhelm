# A failed rotation assertion invalidates subsequent tests’ credentials

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failure after credential rotation could break later tests' authentication.

## Details

F233 — **possible** — `e2e/tests/auth.spec.ts:280` — A failed rotation assertion invalidates subsequent tests’
credentials

After rotating the shared credential, several socket, status, prompt, and browser-exchange assertions can fail before
the replacement credential is persisted. Subsequent workers then use the revoked value. Failures after successful
persistence do not have this issue, and operator credentials are outside the demonstrated scope. Recover and persist a
usable harness credential unconditionally after rotation, preserving the original assertion failure.

## Evidence and triage context

- e2e/tests/auth.spec.ts:34-42 explicitly targets the owned stack's state directory.
- e2e/tests/auth.spec.ts:280 rotates the token; :281-301 contains multiple assertions before the replacement exchange at
  :330-332.
- e2e/tests/auth.spec.ts:356-371 persists the replacement only on the successful forward path; the test ends at :395
  without unconditional credential recovery.
- e2e/tests/helpers/device-auth.ts:27-58 reads the persisted credential for later workers; e2e/playwright.config.ts:23
  and :108-109 use it for request headers and browser storage.
- SPEC.md:1894-1899 and SPEC_impl.md:2707-2713 confirm that rotation invalidates existing browser credentials.
- e2e/tests/auth.spec.ts:280 rotates; :281-301 can fail before the exchange at :330-332 and the write at :360-371. There
  is no unconditional recovery before the test ends at :395. e2e/tests/helpers/device-auth.ts:27-58 subsequently reads
  the unchanged file. Same finding as test_infrastructure_05_cor:p1:F2; definite harness defect, not an
  operator-credential compromise.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:6313-6330 accepts losing a pending browser action's report during sign-in. Its consequence and
  scope differ from leaving the test harness's persisted credential revoked, so it does not cover this finding.

Caveats:

- Requires failure after successful rotation and before persistence.
- Confined to the shared test stack; no operator credential revocation established.
- Later failures after successful persistence do not have this problem.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_cor:p1:F2`,
`test_infrastructure_06_sec:p2:C2`.

- `test_infrastructure_05_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_06_sec:p2:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
