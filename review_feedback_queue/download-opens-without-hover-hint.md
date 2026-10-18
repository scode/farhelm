# A download can open without an opening hint

Reviewed commit: 83b350a19565983982f4669bd20773ce9f95465a

## TLDR

A file link can offer only a download on hover, then open the file when Cmd-clicked after the opening policy changes in
another client.

## Details

The hover shows an opening hint only when its stat reply says `can_open`
(`crates/farhelm-ui/assets/terminal-files.js:309–315`). Every native modifier click still requests opening
(`terminal-files.js:455–475`), and the helm checks the current saved policy at click time
(`crates/farhelm-helm/src/downloads.rs:181–199`). If another signed-in client adds the file type after the stat reply, a
previously download-only link can open. Depending on the type and Mac file association, that could execute
agent-provided content. The landing report in `plans/reports/json-download-open.report.md:111–117` identifies this
timing gap.

Suggested bucket: highest, for the possible execution of untrusted content without the hover's opening offer.
Confidence: likely; the policy-change sequence and platform association were not reproduced. Provenance: trailing review
of the stated commit. `SPEC_impl.md` requires a fresh policy check at click time but does not expressly accept an open
request when hover offered only a download. The UI could request opening only when the current hover result offered it,
while the helm retains its own current-policy check.
