# Tightening ADDRESS in #4737 dropped mismatch warnings for scheme-less hosts and credentials using lookalike hyphens/punctuation or valid RFC 3986 userinfo forms

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

A deceptive terminal hyperlink whose visible label mimics a hyphenated web address without a scheme (such as
`my‐bank.com/login` or `my‐bank.com@other.test/login`, using a Unicode hyphen character that looks identical to `-` in a
monospace terminal font), or uses related lookalike punctuation or valid multi-colon / password-only credentials before
`@`, no longer triggers the prominent destination-mismatch warning, whereas it did before commit #4737.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-grid-model/src/link_disclosure.rs:23`.

- Before commit `5d8febf1dc83b72443b8736f130bcb441152cb40`, `ADDRESS` in
  `crates/gui/farhelm-grid-model/src/link_disclosure.rs` used `[^%s/]+\.\p{L}{2,}(?::[0-9]+)?/`, which matched
  scheme-less dotted hosts and userinfo containing lookalike hyphens, other non-ASCII punctuation, password-only
  userinfo, or multiple colons in userinfo, triggering the mismatch warning.
- Commit `5d8febf1` replaced that pattern with allow-listed character classes (`link_disclosure.rs:20-26`): userinfo
  `[\p{L}\p{N}._%+~!$&'*,;=-]+(?::[\p{L}\p{N}._%+~!$&'*,;=-]*)?@` and host `[\p{L}\p{N}_-]+`. `address_shape`
  (`link_disclosure.rs:70-80`) normalizes lookalike colons (`02d0, 02f8, 2236, a789, ff1a, fe13, fe55`), slashes
  (`2215, 2044, 29f8, ff0f, 2571`), dots (`3002, ff0e, ff61, 2024`), and (after `f17d6d71`) `@` (`ff20, fe6b`), but
  misses the following siblings:
  1. **Unicode lookalike hyphens/dashes/minus** (`U+2010` HYPHEN `‐`, `U+2011` NON-BREAKING HYPHEN `‑`, `U+2212` MINUS
     SIGN `−`, `U+FE63` SMALL HYPHEN-MINUS, `U+FF0D` FULLWIDTH HYPHEN-MINUS): both userinfo and host allow only ASCII
     `-` (`U+002D`). Labels like `my\u{2010}bank.com/login` or `my\u{2010}bank.com@other.test/login` pointing to
     `https://other.test/login` return `address_shape == None` and `mismatch == false`.
  2. **Sibling small/fullwidth punctuation**: `U+FE52` SMALL FULL STOP (`﹒`, direct sibling of mapped `U+FE55` SMALL
     COLON and `U+FE6B` SMALL COMMERCIAL AT) and `U+FF3F` FULLWIDTH LOW LINE (`＿`). Labels like `a\u{fe52}b.test/x` or
     `my\u{ff3f}site.test/x` warned before `#4737` and now return `false`.
  3. **Valid RFC 3986 / WHATWG userinfo forms**: empty-username userinfo (`:secret@example.test/x`, rejected because `+`
     requires a non-empty username before `:`) and multi-colon passwords (`example.test:443:x@other.test/x`, rejected
     because `(?::...)?@` allows at most one `:`). Both warned before `#4737` and now return `false`.
- Unlike leading ASCII brackets/quotes (`<https://example.test/>`), which commit `f17d6d71` intentionally excluded,
  visually identical hyphens/punctuation inside a domain or valid credentials before `@` were not meant to stop warning
  (commit `5d8febf1` explicitly aimed to "preserve warnings for addresses with lookalike letters or a familiar site name
  before an @").
- How to verify: call `mismatch("my\u{2010}bank.test/login", "https://other.test/login")`,
  `mismatch("my\u{2011}bank.test@other.test/login", "https://other.test/login")`,
  `mismatch("example\u{fe52}test/x", "https://other.test/x")`, and
  `mismatch("example.test:443:x@other.test/x", "https://other.test/x")` — all return `false` instead of `true`.
- Fix: map Unicode hyphen/minus lookalikes (`\u{2010}`, `\u{2011}`, `\u{2212}`, `\u{fe63}`, `\u{ff0d}`), `\u{fe52}` ->
  `.`, and `\u{ff3f}` -> `_` in `address_shape` (`link_disclosure.rs:72-79`), and allow `:` freely (or empty username
  before `:`) inside the `@`-terminated userinfo group (`(?:[\p{L}\p{N}._%+~!$&'*,;=:-]+@)?`), keeping `Url::parse` on
  the original unnormalized text so any lookalike or mismatched credential still triggers the warning.

Reviewer metadata (g58_osc8_link_disclosure/f3): suggested bucket: high (Surface uncertainty: like f2, this is a
regression that silently suppresses the prominent OSC 8 mismatch warning for visually identical hyphenated-domain
lookalikes and credential spoofs, though the ordinary hover display still shows the real destination host and SPEC.md
lines 3586-3590 state the warning is an aid rather than a guarantee. Surfacing the `highest` vs `high` boundary
explicitly so triage/recording can judge whether this warning regression crosses into `highest`.); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g58_osc8_link_disclosure/f3`.
