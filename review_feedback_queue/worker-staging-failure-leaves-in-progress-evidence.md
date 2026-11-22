# A refused worker staging request leaves an in-progress result

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

A failed worker upload can leave its local evidence directory marked as staging after the command has returned an error.

## Details

`scripts/test-worker/test-worker.sh:214` writes the staging stamp before copying the tools and source. A refusal in
either copy exits before the terminal stamp. Anyone reading the saved directory sees the earlier in-progress state even
though the run never started.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings: `worker-isolation/f11`.
