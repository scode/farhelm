# An open-link card for an unparsable URL shows no site row

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

When an agent asks to open a link whose URL does not parse, or parses without a host, the approval card shows only the
raw URL, without the separate "site" row that otherwise tells the user where the link really goes. Whether such a
request can reach a card at all is not established.

## Details

Suggested bucket: **other** (move to **highest** if such a URL can reach a card and the opener would still open it
somewhere). Confidence: **possible**; reported by the independent drop check of the GUI migration's final review
(2026-11-17) as an unverified lead outside its register.

`crates/gui/farhelm-gui/src/approvals/model.rs:389-399` adds `Fact::Identity("site", site)` only when
`url::Url::parse(url)` succeeds with a `host_str()`; otherwise only `value("URL", url)` is shown. The tests at
`approvals/model_tests.rs:1091-1115` cover parsed URLs with misleading user-info, backslashes and look-alike Unicode,
not an unparsable or host-less one. To verify: check whether the helm's open-url request handling
(`crates/farhelm-helm/src/browser.rs` and the agent request route) refuses a URL without a parsed host before the card
is made; if it does, this is not a bug and the item can be dropped with that citation.
