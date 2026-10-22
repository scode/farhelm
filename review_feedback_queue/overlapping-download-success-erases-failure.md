# Another download's success erases a failure notice

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

When two files are downloading in one terminal, one download's success can erase the other's failure explanation without
another user click. The replacement success notice then fades, leaving no explanation for the missing file.

## Details

`crates/farhelm-ui/assets/terminal-files.js:482–483` permits concurrent transfers with separate controllers. Their
completions share one notice: failure calls `message` at `518–519`, while a different pending transfer can later report
success at `497` or `516`. `message` unconditionally clears the previous notice (`438–450`), and success expires after
five seconds (`455–461`). A failure followed by another already-running transfer's success therefore disappears without
a new click or transfer admission. This is completion ordering, not an old timer firing: `hideMessage` correctly cancels
the old timer.

`SPEC.md:1978–1979` says a failure remains until the next click or download in that terminal. The source comment at
`terminal-files.js:433–435` specifically says the next admitted download. The current replacement regression
(`js-tests/terminal-files.test.js:760–773`) checks a newly started download replacing failure, not a previously admitted
transfer completing after it. Both native completion and browser completion use the shared notice; the browser also
synthesizes a save-anchor click, which does not represent a new user dismissal.

Preserve an outstanding failure when an already-admitted transfer succeeds, or give concurrent transfers separate
outcome ownership. New user clicks or admitted downloads should still dismiss it under the existing contract.

Suggested bucket: `other`. Confidence: confirmed conditional source path; actual browser timing and frequency are
unverified. The consequence established here is a lost diagnostic. No file loss or false success for the failed file is
established. Provenance: B02-file-hover-authority B02-2, with parent source and coverage inspection. No runtime test,
probe or reproduction was performed.

Proposed drop: none. Possible cover: none found in the specifications, Planned bucket, BUGS, queue or ledger. The
lost-diagnostic filter requires a rare trigger; ordinary overlapping transfers do not establish that premise. The
completed timer-cancellation behavior does not cover a later completion replacing the failure.
