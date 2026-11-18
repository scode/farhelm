# Linux notification sounds use the inherited executable path

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Farhelm plays notification sounds with the first `pw-play` on its executable search path. A misconfigured path can
select a different program.

## Details

Haiku pass H-P1-7, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed executable lookup. No ordinary
route for host content to influence lookup was established. An untrusted entry in the launching desktop user's path
would change the security assessment.

- Verdict: NOT CRITICAL
- Route checked: `crates/farhelm-desktop/src/sound.rs:95-97,125-129,139-148,245-259`; SPEC.md:2684-2692.
- Reason: GPUI added this native player in place of browser Web Audio. It runs at the desktop user's authority, searches
  the inherited executable path to support non-system installs, and receives only the app's WAV file path. An untrusted
  writable PATH entry would make this a real command-resolution hazard. No shipped route was found that lets host or
  approval content control the executable search, working directory, or player argument.

Decide which installed player locations to support and resolve from that list or document and check the trusted
launch-path assumption. Preserve support for non-system installs.

Possible cover: the launch environment belongs to the desktop user. SPEC.md:2684–2692 describes when sounds play and
leaves executable resolution open.
