# A recently modified image.png gets a generated attachment name

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A mixed paste could silently omit a distinct fresh image file.

## Details

F297 — **possible** — `crates/farhelm-ui/assets/terminal.js:2134` — A recently modified image.png gets a generated
attachment name

A recently modified file with the placeholder image name is classified into the generated-image bucket. When another
ordinary file is present, file flavor wins and uploads only that bucket, omitting the distinct image without an error.
Supported clipboard production of this mixed payload remains unverified, and the original file is not deleted. Preserve
all distinct real file objects across flavor selection or explicitly report any omission.

## Evidence and triage context

- crates/farhelm-ui/assets/clipboard-name.js:69–79 returns null for an image with the placeholder basename and a recent
  lastModified value. crates/farhelm-ui/src/attachments.rs:160 sets the freshness window to 5,000 ms.
- crates/farhelm-ui/assets/terminal.js:2125–2134 converts that null into generated=true and puts the actual File in
  payload.images, while an ordinary second file goes into payload.files.
- crates/farhelm-ui/src/attachments.rs:341–349 makes file flavor win whenever a file exists; terminal.js:2186–2190
  applies that classification to the combined payload.
- crates/farhelm-ui/assets/terminal.js:2414–2421 sends only payload.files for file flavor. The distinct image receives
  neither an upload nor an omission error; lines 2447–2449 suppress default paste handling.
- SPEC.md:1289–1295 defines actual clipboard file objects as file references; line 1316 forbids silent attachment
  disappearance.
- crates/farhelm-ui/src/attachments.rs:315–333 acknowledges a name heuristic but claims the cost is only a name and that
  distinct real files are all uploaded. That comment is not specification acceptance of the traced omission.
- No matching Planned, BUGS, queue or ledger acceptance was found. The original local file is not deleted. Retain as
  possible, would_surface, with priority unresolved pending confirmation of the clipboard payload premise; do not label
  it definite destruction of stored user data.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_05_cor:p1:C8`.

- `ui_desktop_05_cor:p1:C8`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed:
  not separately tagged in candidate list.
