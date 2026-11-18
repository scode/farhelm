# Attachment paths can paste directory control characters

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Uploading from a session can paste control characters from the host's attachment directory into the same terminal along
with the path.

## Details

Haiku pass H-P8-7, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed retained controls in parent
components. The basename and paste-closing controls have guards. No effect on a different host's terminal or loss of
work was established. A supported path to command submission or cross-terminal input would change the priority.

- Verdict: NOT CRITICAL
- Route checked: `terminal/attachments.rs:207–215,337–345`, `farhelm-supervisor/src/attachments.rs:129–143,164–180`,
  `farhelm-supervisor/src/service/uploads.rs:938–983,1294–1332`, and
  `farhelm-supervisor/src/grid_input.rs:228–258,586–663`.
- Reason: The supervisor sanitizes the basename and removes Escape bytes and paste-ending markers. It does not filter
  other controls from parent directory components, which need only be UTF-8, and the paste encoder preserves some
  controls. The path is sent only to the uploading terminal on that supervisor. A normal filename cannot introduce them,
  and no route from a remote supervisor into another host's terminal is established. The old client also used shell
  quoting and Escape filtering.

Reject or safely transform unsupported control bytes before auto-inserting the path, with a notice that still lets the
user locate the uploaded file. Exercise a configured state-directory component with a retained control byte.

Possible cover: SPEC.md, “Attachments,” requires inserting the host-side path and says nothing about control-bearing
directory components. The displaced-input and paste-order filters concern different mechanisms.
