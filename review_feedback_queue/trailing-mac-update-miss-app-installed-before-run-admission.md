# Mac Update can miss an app installed before run admission

Reviewed commit: 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae

## TLDR

Installing the desktop app after Update's reach check but before the run is accepted can leave a separately installed
launchd service where Update should refuse.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:2930`.

SPEC.md:211–217 and 5852–5865 require refusal when the app is installed; SPEC_impl.md:7923–7933 separates
inspection-only planning from run revalidation. Update checks the app through inspect at service.rs:1762–1773, but only
then retains and returns the one-use plan (1775–1804). The shipped Update button submits the plan in a second command
(GUI hosts/update.rs:239–275). Automatic Update also plans before invoking its separate admission method
(automatic.rs:145–160). Manual and automatic runs are claimed only at service.rs:1873 and 1927, respectively. Thus a
pre-claim interval remains even without a visible confirmation dialog. For a late app with no answering supervisor,
confirmation uses the absent branch and executes the saved plan without repeating inspect
(service.rs:2826–2835,2930–2940,2609–2617). The Mac plan installs files and writes/restarts a launchd job
(plan.rs:735–767). ADD alone repeats inspect (service.rs:2785–2816); Uninstall replans under its guard, so neither is
the route reported here. The explicit best-effort exception at SPEC.md:5874–5876 covers switching setup while a run is
already queued or running. Report only an app appearing after the check but before the claim, including while the plan
reply is pending. Bucket: other for the supported-layout violation. No source route was established to overwrite desktop
app files, lose sessions or materially break the desktop install in this scenario. Confidence: source-supported
conditional route, no timing or outcome observed. Possible coverage: mac-setup-confirmation-misses-installed-app (named
fix covers ADD).

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p5`.
