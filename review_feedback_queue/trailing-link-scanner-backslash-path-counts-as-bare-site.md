# A scheme-less address with a backslash path counts as a bare site name

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text such as `github.com\rust-lang\rust` that points at `https://github.com/someone-else/malware` shows no warning,
because the text is treated as a bare site name, which matches any page on that site. The same text written with forward
slashes warns.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer's rule for warning bypasses (2026-11-24); the reviewer suggested `other`
because the spoof stays on the destination's own site.

### Reviewer record (`g01-scanner-src/f3`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:732`. Summary: a scheme-less address with a backslash
path (`github.com\org\repo`) counts as a bare site name and matches any target on that host

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(scan.rs:727-732, 849; compare.rs:67).

TLDR: Link text such as `github.com\rust-lang\rust` that points at `https://github.com/someone-else/malware` shows no
warning, because the text is treated as a bare site name, which matches any page on that site. The same text written
with forward slashes warns.

Details: `Scanner::accept` sets `structured` only for clusters whose base is `:`, `/`, `?`, `#` or `@` (scan.rs:732) and
derives `host_only: !structured` (scan.rs:849). For addresses without a written scheme (Host and Www anchors)
`backslash_ends_authority` is true and they are parsed by prefixing the destination's scheme (compare.rs `parse`), where
the WHATWG URL parser treats `\` exactly like `/` in http(s). So for `github.com\rust-lang\rust` the extent is the whole
token, `structured` stays false, `host_only` is true, and `matches` (compare.rs:67) compares only `url.host_str()` with
the target's host: any github.com destination matches. `Address::host_only`'s doc (link_claims.rs:96-100) and SPEC.md
('A bare site name with no path (`github.com`) matches any target on that host') both describe a name with no path; to
the URL parser this one has a path. Fix: include `\\` in the `structured` set (at least when
`backslash_ends_authority`), and add `("github.com\\x", false)` to the host_only table at tests.rs:1075. Verify by
reading scan.rs:727-732 and 849, and compare.rs `parse`/`matches`.

Suggested bucket: other. The spoof is confined to the destination's own host (another repository or page on the same
site) and needs backslash paths, which few labels use; it is a correctness gap in the warning rather than a cross-site
bypass.

Confidence: confirmed (source establishes the structured set and that the url crate treats backslash as a path separator
for special schemes).

proposed_drop: none.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f3`.
