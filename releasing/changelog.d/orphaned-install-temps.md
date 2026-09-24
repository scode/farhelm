---
kind: fixed
---
Interrupted installs no longer leave payload-sized hidden staging files behind forever. A later install removes only
Farhelm's own abandoned staging files in the exact managed destination, while unrelated files remain untouched.
