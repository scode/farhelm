---
kind: fixed
---

Terminal hyperlinks with descriptive labels no longer show a mismatch warning just because the label includes a URL in parentheses. Concealed text inside a program's hyperlink no longer affects the warning or responds to hovering and clicking. A web address printed in the terminal now ends where text too faint to read against its background begins, as it already did at concealed text, so clicking it opens only the part you can see.

A label consisting of an address inside matching brackets or quotes still warns when it points elsewhere. If the address matches, the hover shows the ordinary destination preview.

"Too faint to read" means a contrast below 1.35:1 with the background as Farhelm draws it, for example ANSI black on the default dark background or text in its own background color. Gray (bright black) and dim text stay readable.
