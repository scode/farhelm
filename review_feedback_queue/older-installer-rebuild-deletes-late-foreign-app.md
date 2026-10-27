# Older installer rebuild can delete a late foreign app

Reviewed commit: 918eea3a7613d74fd8deb2886ced1d5c542626d3

## TLDR

Installing an older release can delete an unrelated app moved into Farhelm's usual location after the installer's
initial ownership check. This conditional path needs neither an interruption nor failed restoration.

## Details

The production handoff in `scripts/install.sh:932–975` executes the selected older installer. The published v0.27.0
script matches `scripts/fixtures/install-v0.27.0.sh` and historical commit `9689d0d2eeded2f08d172a210f23e5eec0fcaade`.
Its initial inspection selects rebuilding when the app is absent (`1200–1210`). After preparing the replacement, it
moves whatever now occupies that location into its private backup, publishes the replacement and recursively deletes the
backup (`1277–1294`). It never authenticates the late occupant. Replacing an initially approved legacy bundle supplies
the same gap.

Suggested bucket: `highest`, for an established conditional route to deleting a user-owned app and its contents.
Confidence: confirmed source composition and served-source correspondence; occurrence and actual loss were not measured.
No installer or filesystem experiment ran.

Possible cover: `SPEC_impl.md:4675–4683` at trailing tip `a2c2f8861418f8222eca096c797b9f31585ecc1a` accepts the modern
rebuild's narrower interruption/failed-restoration exception. The historical path deletes after an ordinary successful
publication, so those triggers do not match. `SPEC.md:443–454` preserves foreign public entries and grants contents
ownership only after recognition. Selecting an older installer alone is not older state carried across an upgrade. No
whole-finding exclusion is proposed.

Provenance: `a002/discovery-foreign-rebuild`. Check both initially absent and approved-legacy cases when addressing
this; a repair to modern identity checking alone does not change the executed historical script.
