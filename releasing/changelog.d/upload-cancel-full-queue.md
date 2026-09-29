---
kind: fixed
---

Deleting a session while a file was being uploaded to it could take about a minute to finish when the uploading browser
tab had stopped keeping up with the connection. The delete now stops that upload straight away.
