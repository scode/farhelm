"""Declare captures for new release-note entries here.

For v1.2.3, define an `open_control(bridge)` that opens the feature and waits for its stable `id:` target. Add
`Shot("v1-2-3-new-control", open_control, ["id:new-control"])` to SHOTS and list `release-notes/v1-2-3-new-control`
in `website/src/release-notes/v1.2.3.json`. The listing supplies the entry's PR number and alt text.

If the shot needs another host, session or template, extend `docs/docs-shots/scenario.json5` with invented data
that preserves the existing shots. If the engine cannot stage that state yet, extend its staging against the
private fleet before adding the shot. The capture skips images that are already published.
"""

from docs_shots import Shot

SHOTS: list[Shot] = []
