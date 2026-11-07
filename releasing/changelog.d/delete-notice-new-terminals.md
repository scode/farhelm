---
kind: none
---

Nothing user-facing: a Delete of a session in a terminal host now sends its attached viewers the deletion notice, as
for old terminals, instead of sometimes the stream's own "terminal ended" notice first. The app shows neither notice's
text (both reach the page as an ordinary detach), so nothing changes for someone running Farhelm; API clients, logs and
the tests see the corrected notice.
