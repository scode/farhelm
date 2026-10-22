# A trimmed wrapped path becomes a different relative file link

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

When scrollback discards the beginning of a wrapped file path, its surviving tail can become a download link to a
different file in the session's directory. The hover still discloses that resolved file and requires an explicit click.
The deployed wrapping and retention behavior has not been observed.

## Details

Found by persistent reviewer B01-file-recognition, B01-f2, and independently assessed by static-check-26. Suggested
bucket: other. Confidence: source-confirmed recognition mechanism; likely end-to-end behavior with the terminal
retention premise unverified. No security, undisclosed transfer, overwrite or loss of work is established.

`crates/farhelm-ui/assets/terminal-files.js:127–156` stops its backward scan at row zero even when that row remains
`isWrapped`, then tokenizes the surviving continuation as a complete logical line. In the bundled
`assets/vendor/xterm.js:1`, `CircularList.recycle` advances the first index and `BufferService.scroll` replaces the
recycled last slot, leaving the surviving first row's wrapped flag intact.

For an unexecuted example, a path wrapping between `/srv/build/` and `report.txt` can lose the former row and recognize
the latter as a relative path. The supervisor anchors it to the session directory
(`crates/farhelm-supervisor/src/service/downloads.rs:50–76`). If that different file exists and is readable, the lookup
can succeed. This creates a fresh suffix link; it does not reuse an old cached absolute target.

The frontend's fresh lookup and full disclosure at `terminal-files.js:294–345`, unchanged-text and recognition checks,
geometry check and explicit click at `462–493` still apply. Download uses the disclosed canonical path. A hostile
program can already print that relative path directly; this defect supplies no extra authority.

Possible cover: SPEC.md:1803–1829 and SPEC_impl.md:1267–1270 accept finite retention, while SPEC.md:1957–1989 and
SPEC_impl.md:2220–2225 describe visible-path recognition. They do not clearly settle treating a known continuation as a
fresh token. The reconstruction comment at `terminal-files.js:120–125` specifically explains avoiding truncated tokens
whose apparent start changes recognition. Vendor upgrade-only policy is not exact coverage because Farhelm ignores a
wrap flag the vendor supplies. The stale-file-link and numeric-version ledger entries concern different mechanisms. No
exact Planned, BUGS, queue or ledger coverage was found; no rare-glitch filter match was established. Proposed drop:
none.

A possible fix is to refuse reconstruction when the earliest available row remains wrapped. No discarded text needs
recovering. Review and disposition used source inspection only; no reproduction was run or requested.
