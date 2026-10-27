# Fallback installer locking retains a parent-replacement source gap

Reviewed commit: b92b06824645734a8fc17b115fa3f7fa2e8080f3

## TLDR

Replacing the installation's parent directory could separate the held directory from its fallback lock. A supported
sequence causing conflicting work on the same app remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/mod.rs:180–185` opens the held directory, while `208–215` opens the fallback
lock through the pathname. The mismatch is source-established. Missing premises are a supported replacement topology,
conflicting operations reaching the same app or public link despite other guards, and resulting harm. This is not a
reproduced serialization failure.

Suggested bucket: `other`; a security or destructive route would require those missing premises. Confidence: conditional
source lead, independently retained by `a016/f1`. Possible cover: deliberate-interference and writable-directory
exclusions do not unambiguously settle incidental replacement; the interrupted-rebuild exception has a different
trigger. No whole exclusion is proposed. Provenance: a006's discovery fallback-parent lead, `a016/f1`. No runtime
experiment ran.
