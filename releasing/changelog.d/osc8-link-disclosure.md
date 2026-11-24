---
kind: fixed
---

Hovering a program's link in the terminal now warns whenever its text mentions a web address that goes somewhere else, wherever the address appears: in a sentence, in Markdown, in brackets or quotes, or after a ticket number. It also warns when the address uses look-alike or invisible characters, puts login details in front of the real site, or when part of the link's text is concealed or too faint to read, or the whole text couldn't be checked. The warning lists each reason and shows the address it found above the real destination; when that address plainly names another place, the part of the destination that differs from it is highlighted. Email addresses, file names and decoration such as quotes, arrows and trailing punctuation don't trigger it, and neither do words around an address that matches. The hover also shows a link's whole destination now, however long, instead of leaving out its middle. A very long destination may not fit in a small window; clicking the link is then refused until you enlarge the window.

The comparison is strict: `http` against `https`, an added `www.`, an extra query and a shortened address all count as different. A bare site name such as `github.com` matches any page on that site.

Text too faint to read means a contrast below 1.35:1 with its background as Farhelm draws it, for example ANSI black on the default dark background or text in its own background color; gray (bright black) and dim text stay readable. Such text, like concealed text, is left out of a link's text, its hover and click area. A web address printed in the terminal ends where such text begins, as it already did at concealed text, so clicking it opens only the part you can see.
