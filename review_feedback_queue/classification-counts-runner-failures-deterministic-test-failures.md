# Classification counts runner failures as deterministic test failures

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The failure classifier treats runner errors as proof of deterministic test failure.

## Details

F256 — **definite** — `deflake/bin/deflake:1274` — Classification counts runner failures as deterministic test failures

Every nonzero recorder exit counts as a failed observation of the selected test, even when infrastructure failure
prevented that test from executing. Three such attempts can manufacture a deterministic-failure verdict and direct
investigation using unsupported evidence. Require completed structured outcomes for the selected test, retaining
infrastructure failures as unclassified attempts.

## Evidence and triage context

- deflake/bin/deflake:1204-1217 extracts initial test failures from structured reports.
- deflake/bin/deflake:1233-1240 distinguishes unnamed initial infrastructure failures from tests eligible for
  classification.
- deflake/bin/deflake:1259-1278 reruns the selected test but computes the verdict solely from command status and attempt
  count.
- scripts/record-test-run.py:1855-1877 returns 125 for recorder errors independently of a test result.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- deflake/SPEC.md:39-47 requires repeated test observations and explicitly distinguishes runner failures.
  FILTER.md:24-42 does not establish that repeated setup/refusal failures are a rare trigger; this is a classification
  error, not merely missing diagnostic detail.

Caveats:

- Scoped to selected-test reruns. Monolithic battery classification is a separate contract.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_cor:p2:F3`.

- `test_infrastructure_04_cor:p2:F3`: confidence as filed: definite; suggested bucket as filed: other.
