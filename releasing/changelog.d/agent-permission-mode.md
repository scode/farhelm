---
kind: changed
---

Saved agent sessions with no recorded permission now show the YOLO shield and ask for the same confirmation on Replace.
New Cursor sessions default to YOLO and still accept Custom flags.

Replace offers **Replace as guarded instead** for YOLO sessions whose agent supports Guarded. Confirming a plain YOLO
replacement keeps the saved command.

Caveat for curation: because the upgrade records YOLO on old agent sessions that never chose a permission, **Clone**,
**Replace with** and **Restart with** of such a session now start from YOLO and pass the agent's bypass flags, where
they used to start a Guarded launch for agents that offer it. A host that asks before YOLO launches still asks and
offers to start guarded instead; a host set to start YOLO sessions without asking starts it as YOLO. Choosing Guarded in
the launcher still starts a guarded session.
