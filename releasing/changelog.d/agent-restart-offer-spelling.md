---
kind: fixed
---

The instructions Farhelm gives agents now say how the restart offers in `farhelm agent sessions --json`
(`fallback_template`, `fresh_only`) map to `farhelm agent restart --mode` (`fallback-template`, `fresh`). An agent that
copied the offer from the JSON into `--mode`, as the instructions told it to, got a usage error instead of a restart.
