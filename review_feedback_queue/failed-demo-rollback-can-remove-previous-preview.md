# A failed preview rollback can remove the previous demo output

Reviewed commit: 53214d2c0079ba50081f47a5066c7732c651a4c7

## TLDR

If the demo recorder cannot restore a previous movie, marks file, or still during rollback, the previous output can be
lost when the recorder cleans up its temporary files.

## Details

The recorder puts backup files under the temporary directory that holds the new output. The rollback callbacks move
those backups back to their original paths. If a move back fails, exiting the temporary directory can delete the
unrestored backup. The failure report identifies the separately retained capture run, which does not contain those
output backups.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `scripts/readme_video.py:212–248` and
`scripts/readme_video.py:525–533`. The existing test checks a failed forward rename followed by successful restoration
in `scripts/test-readme-video.py:72–101`. Retain unrestored backups when rollback fails and report their directory.

Suggested bucket **other** for conditional loss of generated maintainer output. Source-confirmed by trailing reviewer
g014/l25.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g014/l25.
