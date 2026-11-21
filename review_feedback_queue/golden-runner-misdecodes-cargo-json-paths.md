# Cargo artifact parsing can reject a valid non-ASCII build path

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A successful golden build can fail before rendering when Cargo reports a path the runner decodes or splits incorrectly.

## Details

scripts/check-gui-goldens.py:90–107 captures Cargo's artifact stream in text mode without an encoding, then applies
str.splitlines() before json.loads. The Cargo Book, External tools 'JSON messages' and 'Artifact messages', defines JSON
objects separated by newlines and includes path-valued fields such as executable, filenames, and manifest_path. The
reference cargo_metadata parser uses BufRead::read_line (src/messages.rs:268–284). A UTF-8 JSON object with a literal
non-ASCII path is valid. Serde_json's default format writes non-escaped fragments as UTF-8 (src/ser.rs:1757–1762 and
:2079–2169). With a Python process configured for a strict ASCII locale with UTF-8 mode disabled, the Python 3.12
subprocess defaults use locale.getencoding() and strict decoding (subprocess.py:361–378, :875–877 and :1016–1020). A
successful build with a literal non-ASCII path in any artifact object then raises UnicodeDecodeError before the loop
even identifies farhelm-gui's executable. The same parser also treats Unicode line separators as record boundaries even
inside JSON strings, whereas Cargo's documented records are split at actual newlines. A custom target directory is an
accepted option (scripts/test-check-gui-goldens.py:80–99). No UTF-8 Python locale restriction is documented. The
previous scripts/check-gui-goldens.py:116–125 at dbb42b527d2ebd7918aaa2f4e0c87f06f78bcc47 lets Cargo handle the build
and does not decode its stream. Parse the captured bytes as newline-delimited UTF-8 JSON, splitting only on LF, or
specify UTF-8 and use the same LF-only delimiting.

Confidence: confirmed by inspection of the parser and format contracts, conditional on Cargo emitting an artifact record
with a literal non-ASCII path and, for the locale failure, Python using an incompatible decoder.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `goldenRunner/l10`.
