---
kind: removed
---

Links that a program prints under text of its own (OSC 8 hyperlinks, such as `ls --hyperlink` prints) now do nothing in
a session terminal unless they go to a web address. A `file://` link no longer offers a download: hovering it shows no
file details and clicking it saves nothing. To download a file, have the program print its path, which works as before.

Other schemes, such as `ssh://`, already opened nothing; they now also show no underline or pointer. A program that
handles the mouse still gets clicks on these links. Text right next to any hyperlink, web links included, is no longer
joined with the link's text into a downloadable path either, so `/tmp/` printed before a link labelled `report.pdf`
offers nothing.
