# Startup can hide a supervisor ownership-record failure

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Farhelm can start its supervisor and then silently fail to record it. Later launches can treat that supervisor as
externally started.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

When the app starts its supervisor but fails to save ownership of it, startup proceeds and only the log says that later
launches will treat the supervisor as externally started. Give the window a nonfatal diagnostic with the reason and
location of the failed write.

Reviewed locations: `crates/farhelm-desktop/src/lib.rs:1308-1316`. Haiku out-P1.md §3. Suggested bucket **other**.

SPEC_impl.md:7220–7236 describes the ownership record and later reuse. SPEC.md “Healthy local filesystems” accepts local
storage failure. The proposed diagnostic is about the later ownership consequence despite successful startup.
