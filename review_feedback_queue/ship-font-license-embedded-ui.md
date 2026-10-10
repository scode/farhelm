# Ship the font license with the embedded UI

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Release bundles could omit the license required with embedded fonts.

## Details

F216 — **possible** — `crates/farhelm-ui/src/lib.rs:1160` — Ship the font license with the embedded UI

The fonts are registered for bundling, but their full license is not; internal metadata contains only a licensing
statement and URL. A produced release or another inclusion mechanism could still refute the omission, so no definitive
distribution violation is established. Inspect a release bundle, and if the notice is absent, retain and register the
full license alongside the fonts using the existing license-asset pattern.

## Evidence and triage context

- crates/farhelm-ui/src/lib.rs:1161 and :1166 register the regular and bold font files. The reference to OFL.txt at
  :1159 is a comment, not an asset declaration.
- crates/farhelm-ui/src/lib.rs:998-1019 generates all_assets from declared assets. The existing retained license asset
  at :1111-1115 demonstrates the separate declaration used to carry a notice into bundles.
- crates/farhelm-ui/assets/app.css:80-83 and :100-103 reference the registered font files;
  crates/farhelm-ui/assets/terminal.js:3499 selects the same CSS family.
- crates/farhelm-ui/src/desktop/assets.rs:71-74 serves desktop assets from the embedded UI directory.
  crates/farhelm-helm/build.rs:62-73 explains that this directory is embedded wholesale, leaving its actual contents as
  the remaining release-artifact verification premise.
- dist-workspace.toml:231-235 disables source tarballs and automatic inclusions and lists LICENSE as the additional
  static file. packaging/farhelm-desktop/dist.toml:57 lists the desktop binary and icon;
  scripts/build-desktop-binary.sh:120 and :128 stage those artifacts.
- crates/farhelm-ui/assets/fonts/OFL.txt:56-61 requires each distributed copy to contain the copyright notice and
  license, allowing readable metadata as an alternative to a separate file.
- Independent read-only decoding of both app WOFF2 files found name records 0, 13 and 14 containing copyright, a short
  OFL statement and an OFL URL, but not the full license.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:721-725 specifies shared vendored fonts, but does not accept omission of their license.
- TODO.md:24-31 and TRIAGE_OUTCOMES.md:5977-5991 concern font-loading fallback and retained terminal instances. Their
  trigger, consequence and scope differ from missing distribution notices.
- review_feedback_queue/font-focus.md:14-20 concerns keyboard focus after font-size clicks, not font packaging.
- TODO.md:33-43, BUGS.md and the inspected queue/ledger searches provide no matching accepted disposition.

Caveats:

- This is distribution-artifact correctness with a licensing consequence; no rendering failure, security exploit or
  user-work loss is established.
- The actual release bundle and bundler implementation were not inspected. An additional inclusion mechanism could
  refute the omission.
- Retained because the allegation concerns a required shipped artifact, not pure documentation drift. Its
  compliance-oriented scope should remain explicit during triage.
- No checkout writes, runtime tests, builds, fixes, VCS mutations or nested agents were performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `binary_cor:p1:F1`.

- `binary_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
