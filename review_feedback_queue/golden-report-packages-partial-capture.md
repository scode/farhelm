# A failed renderer can put an unfinished screenshot on the review page

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A failed screenshot renderer can leave a partial PNG that the changes page labels as a new capture.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/goldens/mod.rs:1612-1650;
crates/gui/farhelm-gui/src/goldens/changes.rs:95-125; scripts/check-gui-goldens.py:220-244,309-345;
scripts/golden_changes_page.py:44-65,126-169; scripts/test-check-gui-goldens.py:494-545; image 0.25.10
src/images/buffer.rs:1045-1059 and src/io/free_functions.rs:46-57

Both report modes can package an unfinished screenshot as New capture when a render worker stops after opening the final
PNG but before encoding completes. The locked image library creates the final path before emitting PNG bytes. An
ordinary nonzero worker exit, including SIGKILL, preserves the failed status but still reaches page generation. Compare
only saves pictures for differing, missing, or unreadable references and writes review metadata after saving the
capture. If saving was interrupted, the page still copies the matching filename with no metadata. If saving finished and
only highlight writing was interrupted, the saved image can be valid and the fallback is appropriate. Update saves every
rendered capture and treats a partial file as differing by byte identity, or as new if it has no manifest ID. For a
paired update row, the later native review logs an undecodable capture and skips its metadata, but row construction
still presents the file as New capture. An update without a paired reference does not attempt this decode. Absence of
metadata is shown only as missing pixel comparison on a paired row; it does not identify a broken capture. A native
review worker only reads captures, so its failure cannot create a partial capture. The original nonzero render status
still makes compare fail and blocks update certification. The killed-worker test asserts diagnostics, retained filenames
and the absence of a publication marker, and does not inspect page contents. Keep complete outputs available after a
failure and expose a PNG under its final scan-visible name only after encoding and write completion. Introduced at
0d52ff68e92cd2bee5b4752d02c3a3210ffbdd5d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `golden-report/GR-08`.
