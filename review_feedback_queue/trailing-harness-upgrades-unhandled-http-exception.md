# Fallback installer download crashes drift runner on http.client.HTTPException

Reviewed commit: f2e5f1679bb902c7227cc2cc0dfddf893e9885fc

## TLDR

When upgrading an agent CLI falls back to downloading its vendor installer (used for Goose and Cursor) and the download
hits a truncated response or malformed HTTP reply, the drift test runner crashes with an unhandled traceback instead of
recording that agent's upgrade as failed and continuing with the remaining agents.

## Details

**other.** Source inspection. Main reference `scripts/harness_upgrades.py:124-133`.

In `scripts/harness_upgrades.py:124-133`, `run_installer()` calls `urllib.request.urlopen(url, timeout=VERSION_TIMEOUT)`
and `response.read()` inside `except OSError as error:`. In Python's standard library, `http.client.HTTPException`
(including `http.client.IncompleteRead` from `response.read()` and `http.client.BadStatusLine` / `LineTooLong` from
`urlopen()`) does NOT inherit from `OSError` (verified via `issubclass(http.client.HTTPException, OSError) == False`).
`upgrade()` (`scripts/harness_upgrades.py:139-151`) documents 'Never raises for a vendor-side failure', and `prepare()`
in `scripts/run-harness-drift.py:174-191` calls `harness_upgrades.upgrade(recipe, env)` without exception handling.
Consequently, an HTTP framing or truncated-body error during a fallback installer fetch aborts the entire drift run
before upgrading or running any remaining harnesses. Verify by inspecting `scripts/harness_upgrades.py:129-132` and
Python's `http.client.HTTPException` MRO. Fix by catching `(OSError, http.client.HTTPException)` in `run_installer()`
(and optionally bounding the download size).

Reviewer metadata (g12_tooling_drift_upgrade_and_worker/f1): suggested bucket: other (maintainer drift-test tooling
crash on transient HTTP error; no user data or security impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g12_tooling_drift_upgrade_and_worker/f1`.
