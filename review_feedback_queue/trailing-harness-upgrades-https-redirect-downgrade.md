# Fallback installer fetch allows HTTPS-to-HTTP/FTP redirect downgrade before executing with bash

Reviewed commit: f2e5f1679bb902c7227cc2cc0dfddf893e9885fc

## TLDR

When the drift test runner falls back to downloading a vendor's shell installer and executing it with bash on the
maintainer's machine, the downloader silently follows redirects to unencrypted HTTP or FTP, allowing a redirect
downgrade combined with network interception to execute attacker-controlled shell code with full workstation access.

## Details

**other.** Source inspection. Main reference `scripts/harness_upgrades.py:124-134`.

In `scripts/harness_upgrades.py:124-134`, `run_installer()` fetches `recipe.installer_url` (`https://cursor.com/install`
at line 86 or `https://github.com/aaif-goose/goose/releases/download/stable/download_cli.sh` at line 67) via
`urllib.request.urlopen(url, timeout=VERSION_TIMEOUT)`, writes the response bytes to `install.sh`, and executes
`bash install.sh`. Verified in Python's stdlib (`urllib.request.HTTPRedirectHandler.http_error_302` checks
`urlparts.scheme not in ('http', 'https', 'ftp', '')`): the default opener silently follows redirects from `https://` to
`http://` or `ftp://`, and `run_installer()` never rejects non-HTTPS redirects or checks `response.geturl()` before
invoking `bash`. By contrast, `curl --proto '=https' --tlsv1.2` refuses protocol downgrades. Fix by installing a custom
`HTTPRedirectHandler` (or `urllib.request.build_opener`) that rejects any non-`https` redirect target and asserting the
final URL scheme is `https` before writing or executing `install.sh`.

Reviewer metadata (g12_tooling_drift_upgrade_and_worker/f2): suggested bucket: other — boundary with `highest`
explicitly noted. Running attacker-controlled shell code via `bash` on the maintainer's development workstation (which
holds SSH keys, GitHub/vendor credentials, and the live Farhelm install) would be catastrophic and WOULD cross into
`highest` despite the maintainer-tooling carve-out (`review_feedback_queue/AGENTS.md`, 'Priority buckets': 'stays out of
`highest` unless it is catastrophic'). However, under the established-route rule (maintainer, 2026-10-22), both
hardcoded URLs start on `https://` with TLS certificate verification, and no evidence shows either endpoint currently
redirects through `http://` or `ftp://` (an attacker who already controls the HTTPS endpoint does not need a downgrade).
Therefore the established bucket is `other` (defense-in-depth hardening). What would move this to `highest`: showing
that either configured `installer_url` (or a newly added one) actually redirects over `http://`/`ftp://` on an untrusted
network, or a maintainer triage judgment that unguarded protocol downgrades in `curl | bash` workstation installers
belong in `highest` unconditionally. Because of this uncertainty at the `highest` boundary, surface it during triage and
do not drop without an independent drop check.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g12_tooling_drift_upgrade_and_worker/f2`.
