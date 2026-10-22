# Numeric-version guard hides artifact filenames

Reviewed commit: fb318dd96c596bad4ccc207ae9b945f2b6022c1a

## TLDR

Bare filenames such as `1.2rc1-notes.txt` and `1.2rc1-linux.tar.gz` lose file hover and download actions because Farhelm
treats them as version numbers. Prefixing the path with `./` avoids this omission.

## Details

Suggested bucket: **other**. Confidence: **confirmed source mechanism**, with an unresolved specification boundary. No
file mutation, loss or runtime measurement is claimed.

`crates/farhelm-ui/assets/terminal-files.js:70–75` widens the numeric-version exclusion. Line `74` accepts alphanumeric
text after a dotted digit, then consumes the remaining filename as version metadata before the extension check. Parent
`461d49e6f1ec47e2e657e4461d9100a923331e3e`, the same file at `70–71`, does not classify these examples as versions and
permits their alphabetic extensions. This establishes the newly excluded examples above.

The initial review also mentioned `1.7z` and `2026.7z`. Those were already omitted before this commit and are not new
regressions. Keep that attribution separate from the new artifact-name exclusion.

Possible cover: `TRIAGE_OUTCOMES.md:16537–16544`, under `file-recognition-rejects-digit-leading-extensions.md`,
explicitly excludes version numbers and quantities such as `1.5x`; `SPEC.md:1908–1910` also excludes versions while
recognizing relative filenames with extensions. Neither clearly decides these version-shaped artifact filenames. No
exact Planned, known-bug or queue coverage was found. Proposed drop: none; triage should settle that boundary before
changing conservative recognition.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, corrected `g034/f1`.
