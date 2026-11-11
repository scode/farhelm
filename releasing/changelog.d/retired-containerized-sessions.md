---
kind: breaking
---

Earlier experimental containerized sessions now support only Delete, and their agents keep running until deletion
removes their containers. Templates with the old containerized setting can launch after that setting is removed.
