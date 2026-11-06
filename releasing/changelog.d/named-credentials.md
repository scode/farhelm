---
kind: breaking
---

Settings → Credentials now keeps named tokens and agent logins, with a warning that they are not encrypted on disk.
Tensorlake recipes select named endpoint tokens; existing recipe tokens are removed, so re-enter your endpoint tokens
in Settings → Credentials and choose their names in the recipes.
