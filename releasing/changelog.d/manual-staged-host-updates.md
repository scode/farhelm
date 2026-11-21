---
kind: fixed
---

Farhelm uses locally staged builds for host updates you start yourself. Automatic remote-host updates skip these builds.

Context for curation: this only affects a desktop app started with a locally staged host build directory
(`FARHELM_HELM_PAYLOAD_DIR`), which is how devbuilds run; a release build downloads signed host builds and keeps
updating hosts automatically. The plan's release note section says no release note is needed, so this entry can likely
be dropped.
