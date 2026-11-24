# Zero-width Default_Ignorable characters (notably live-reachable U+1160) bypass INVISIBLE and suppress the OSC 8 mismatch warning

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

A remote terminal program can emit a deceptive hyperlink whose visible label looks identical to a web address such as
`https://github.com/login` or `www.github.com` while pointing elsewhere, yet suppress the prominent mismatch warning by
inserting a zero-width invisible filler character (`U+1160`) into the scheme separator, `www.`, or port. The ordinary
hover disclosure still names the real destination host, so the primary safeguard remains, but the prominent mismatch
warning promised by SPEC.md is bypassed.

## Details

**highest.** Source inspection. Main reference `crates/gui/farhelm-grid-model/src/link_disclosure.rs:32`.

- In `crates/gui/farhelm-grid-model/src/link_disclosure.rs:32`, `INVISIBLE` is defined as `r"[\p{Cf}\p{M}]"`, which only
  strips Unicode Format (`Cf`) and Mark (`M`) characters before matching `ADDRESS` (`link_disclosure.rs:68-81`).
- Exhaustive audit of `unicode-width 0.2.3`'s `lookup_width` table (`src/gen/tables.rs` + `src/lib.rs:100-114`) against
  `vendor/alacritty_terminal/src/term/mod.rs:1442-1550` and `regex-syntax 0.8.11` (Unicode 16.0.0) identifies three gaps
  in `INVISIBLE = [\p{Cf}\p{M}]`:
  1. **Live-reachable invisible filler (`U+1160`, HANGUL JUNGSEONG FILLER):** `U+1160` has Unicode General Category `Lo`
     (not `Cf` or `M`), even though it is a `Default_Ignorable_Code_Point` that draws nothing and is already listed as
     an invisible filler in both `LEADING_BLANKS` (`link_disclosure.rs:35`: `[\u{115F}\u{1160}\u{3164}\u{FFA0}]`) and
     `farhelm_proto::text::is_presentation_unsafe` (`crates/farhelm-proto/src/text.rs:36-38, 85-86, 95, 97`). In
     `vendor/alacritty_terminal/src/term/mod.rs:1502-1550`, `U+1160` is NOT discarded by `emoji_ignored_format`
     (`mod.rs:1442-1450`, which lists `U+3164` and `U+FFA0` but omits `U+1160`), and `unicode-width 0.2.3` assigns
     `U+1160` width `Some(0)` (both `Default_Ignorable_Code_Point` and `Hangul_Syllable_Type=V`). Consequently
     `alacritty_terminal` attaches `U+1160` via `push_zerowidth` to the preceding cell (`:` or `/` or `w`) with zero
     extra columns, and `crates/farhelm-supervisor/src/grid_output.rs:202-219` emits the cell text with `U+1160`
     embedded. Labels like `https:\u{1160}//github.com/login`, `https:/\u{1160}/github.com/login`,
     `www\u{1160}.github.com`, or `github.com:\u{1160}443/login` pointing to `https://evil.test/login` fail all three
     branches of `ADDRESS`, returning `address_shape == None` and `mismatch == false`.
  2. **Legacy-replay (`emoji_width == false`) / wire-level `Default_Ignorable_Code_Point`s:** `U+3164` (HANGUL FILLER,
     `Lo`), `U+FFA0` (HALFWIDTH HANGUL FILLER, `Lo`), `U+115F` (`Lo`), and 3,769 unassigned
     `Default_Ignorable_Code_Point`s (`Cn`: `U+2065`, `U+FFF0..U+FFF8`, `U+E0000`, `U+E0002..U+E001F`,
     `U+E0080..U+E00FF`, `U+E01F0..U+E0FFF`) are all width-0 non-drawing characters in `unicode-width 0.2.3` yet are
     neither `Cf` nor `M`.
  3. **Unicode version skew on newly added combining marks:** `unicode-width 0.2.3` covers Unicode 17/18 combining marks
     (e.g. `U+05C8..U+05C9`, `U+1ACF..U+1AF0`, etc., width 0 and attached by `alacritty_terminal`), whereas
     `regex-syntax 0.8.11` ships Unicode 16.0.0 tables where those code points are `Cn` rather than `\p{M}`.
- This directly contradicts SPEC.md ("Terminal experience", lines 3573-3575, updated in commit `f17d6d71`): "When the
  underlined text is itself a web address (it starts with a scheme or `www.`, or is a dotted host, optionally after
  credentials, ending in a name of two or more letters followed by `/`; characters that draw nothing are ignored for
  this test) and names a different place, ignoring only a trailing slash, the display becomes a prominent warning".
- How to verify: call `mismatch("https:\u{1160}//example.test/x", "https://other.test/x")` and
  `mismatch("www\u{1160}.example.test", "https://other.test/x")` in
  `crates/gui/farhelm-grid-model/src/link_disclosure.rs` — both return `false` instead of `true`.
- Fix: widen `INVISIBLE` to `r"[\p{Cf}\p{M}\p{Default_Ignorable_Code_Point}]"` (verified supported by
  `regex-syntax 0.8.11` `property_bool.rs:22`, whose `DEFAULT_IGNORABLE_CODE_POINT` table includes `U+115F..U+1160`,
  `U+3164`, `U+FFA0`, `U+2065`, `U+FFF0..U+FFF8`, and `U+E0000..U+E0FFF`) — and if desired, also strip any character
  with `unicode_width::UnicodeWidthChar::width(ch) == Some(0)` in `address_shape` so the recognizer stays lockstep with
  `alacritty_terminal`'s zero-width attachment rule across Unicode versions, while keeping `Url::parse` on the original
  unstripped string.

Reviewer metadata (g58_osc8_link_disclosure/f2): suggested bucket: highest (Surface uncertainty: this is a
live-reachable phishing/spoofing warning bypass on OSC 8 hyperlinks that contradicts SPEC.md's "characters that draw
nothing are ignored for this test". However, SPEC.md lines 3586-3590 also state "The warning is an aid, not a
guarantee... and the ordinary display, which always names the real host, is what remains. The hover display is the whole
safeguard". Because a user still sees the ordinary hover display naming the real host before clicking, this could
alternatively be triaged as `high`; surfacing the `highest` vs `high` boundary explicitly per priority-bucket rules.);
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g58_osc8_link_disclosure/f2`.
