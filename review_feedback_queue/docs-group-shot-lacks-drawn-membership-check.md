# The session group screenshot checks saved membership only

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

The release screenshot can pass its checks without confirming that the moved session was drawn under its new group.

## Details

`scripts/docs-shots/release-notes.py:64–77` polls the helm’s stored membership. Its crop at `:130–136` matches the same
row in either position. The GUI receives the change and advances its held row independently of that read. Wait for the
drawn placement as well as the saved association. The missing witness is source-confirmed. Whether the frozen screenshot
shows the wrong placement is unverified after investigation because no image or ordering trace was available.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `docs-capture/l3`.
