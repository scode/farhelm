# File disclosures leave known invisible path characters unescaped

Reviewed commit: a5f5f8213eeb0a1a5161c22cf6260501aca81be9

## TLDR

Different agent-controlled filenames can look identical in the disclosure meant to authorize their download.

## Details

Suggested bucket: `highest`. Confidence: confirmed by code inspection; no browser reproduction. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

The new disclosure at crates/farhelm-ui/assets/terminal-files.js:172–175 uses terminal-links.js:313–317, which escapes
only Unicode Cf characters. U+034F, for example, is an invisible combining grapheme joiner outside Cf;
report<U+034F>.pdf remains visually indistinguishable from report.pdf. Recognition and OSC decoding accept it (:20–24,
:50–52), and the raw resolved path is downloaded at :295. The repository already identifies this character as
presentation-unsafe in farhelm-proto/src/text.rs:28–35, :53–75. CSS isolation does not reveal it. The new file surface
violates SPEC.md:2268–2278: host-supplied GUI text must expose invisible characters, and the path must be disclosed
before transfer. Use the full established presentation-unsafe policy for file disclosures and outcomes.

Possible cover: TRIAGE_OUTCOMES.md, display-peer-misses-invisible-chars.md (:2330–2354), documents an earlier fix to the
shared Rust escaper and installation identities. It does not cover this newly introduced JS file-disclosure surface.

Current-main status: Persists at pinned main: terminal-files.js:175 still calls the same helper;
terminal-links.js:334–337 still escapes only Cf.

The full commit diff and relevant paths were read. No runtime reproduction was run.

## Additional affected scope

The dedicated `gpt-6-astra` high review of the same commit also identified an ambiguity in the escaped presentation:
literal `<U+XXXX>` filename text is preserved and can look like the helper's escaped output, while the disclosure's CSS
collapses repeated whitespace. Distinct raw paths can therefore have the same presented evidence. Suggested bucket:
`highest`; confidence: likely for visual ambiguity, with concrete browser/font rendering unverified. Proposed drop:
none. This is retained scope within the same file-disclosure finding, rather than another distinct discovery.

The reviewed terminal-links.js helper at 313–316 emits literal escape notation without escaping matching literal text;
terminal-files.js:172–175 renders it in the tooltip, and app.css's tooltip rules do not preserve repeated spaces. The
raw target still drives the transfer at terminal-files.js:290–295. The original
installation-identity-escape-collision.md queue item concerns installation adoption identities, not downloaded paths. No
exact acceptance was identified. This presentation remains at the frozen endpoint. No runtime reproduction was run; the
full reviewed commit diff was read and unchanged dependencies were inspected selectively.
