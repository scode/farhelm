## What this was about

The Settings gear looked squashed because its outline was wider than tall. It now uses the approved round, eight-tooth
design with a centred hole. A regression test checks that its outline stays square and centred.

## Things you should know

The approved proportions were used unchanged. The rail gear remains 16px and the host-menu gear remains 14px; colour,
stroke, accessibility and all callers are unchanged. The selectable gear used as a host's identity mark is a different
icon and was left alone.

The plan called the Settings dialog's gear 16px, but existing CSS already enlarged it to 28px. That size was preserved.
Actual app captures were inspected in Chromium and WebKit, in dark and light appearances, at all three existing sizes;
the gear reads cleanly beside Help and in the host menu.

## Open questions and possible follow-ups

None.

## PRs

- [#2826: correct the squashed settings gear](https://github.com/scode/farhelm/pull/2826/changes), draft, based on main.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --lib`: 824 passed, zero skipped, recorder `9e1bbc1c-0b50-4a1d-8289-5a479510eac5`.
  After a lint-only correction to coordinate pairing, the focused geometry test passed again in
  `922fc4e9-e42b-4c34-8164-8b820cb08577`; the other 823 were selected out. Their earlier pass remains applicable because
  their code and product behavior did not change.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`: passed in `7e7e470b-6e4b-4459-9abd-ab6d109bef12`. Initial
  run `9077351d-77fd-4302-ad45-edbd0afa061a` rejected the test's slice-chunk API; the suggested fixed-size API resolved
  it.
- `cargo fmt --all -- --check`, changed-Markdown `dprint check`, changelog format lint and the isolated test-delay
  checker passed. The delay checker found 327 delays and zero missing rationales. An initial formatting wrap was fixed.
- Backend/fixture build `4abb8c18-4585-43ad-b513-4681c27d5d52` and web release build
  `78b213ea-ef55-4701-8fb2-0923fcad9f92` passed. Private Playwright capture `f3835412-528d-41b3-bee3-68aa5a3f2533`
  passed both engine cases, each covering both appearances and all three sizes, against PR head
  `3a7cfddbc6a041f16ec6502ccaea5fe210e6f360`. This used generic recorder mode with a private config; its two passes were
  checked in Playwright's report. All three owned stacks shut down cleanly.
- Earlier capture `15e7ca51-e621-470c-a32e-40844e382c61` stopped before tests because the private config resolved its
  stack script from the wrong directory. After correcting that, `88a4a4dc-bd0a-402d-9606-aa8c62e9e7e4` rejected the
  plan's inaccurate 16px header expectation. Correcting the capture to the existing 28px CSS produced the pass above.
  Both failures remain retained; neither required a product change.
- Runtime evidence was reused after rebasing onto main `3a68d3c57b35a601509fbbb0fcacde2d9069e79d`: incoming work only
  curated release notes and removed old fragments, leaving runtime inputs unchanged. Changelog lint was rerun. The full
  browser suite, desktop shell, installer and unrelated runtime suites were skipped because this changes only static
  inline geometry. No CI run was requested.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues and reaffirmed that conclusion
after the lint correction. The commit/PR wording also passed its fresh cold read. No findings were rejected.
