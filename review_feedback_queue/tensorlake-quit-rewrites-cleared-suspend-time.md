# Quitting during Suspend can restore an already-cleared timestamp

Reviewed commit: 405b3a3dcec6216ed9252d7a5128de649eab8530

## TLDR

Quitting while Suspend finishes can put back an old timestamp that Farhelm had already cleared.

## Details

**other**. Confirmed by source and contract inspection.

Quitting while Farhelm finishes a Suspend can make the next launch show a timestamp from an earlier pause, even after
Farhelm already cleared that timestamp. Confirmed by source scheduling and write order, with no runtime observation.
Explicit Suspend holds the host provisioning lock but captures the binding before parking
(tensorlake/lifecycle.rs:114–148, environment/lifecycle.rs:92–109). An older stored stamp can be read there before the
actor settles a Running probe or an interrupted dial and starts clearing that stamp
(manager.rs:4197–4198,4854–4857,4897–4912,4554–4567). The clear can commit while the parked lifecycle task waits for a
Tensorlake status response. If Tensorlake answers that the sandbox is already suspended, the transition returns false
(tensorlake/lifecycle.rs:197–205). Normal quit drains the actor map before joining actor tasks (manager.rs:3570–3604),
so the detached lifecycle task can resume in that interval and read no Running evidence from the now-empty map
(manager.rs:2427–2430). Lifecycle work is independently spawned (tensorlake/lifecycle.rs:64–69, sessions.rs:3798–3802).
Neither shutdown nor the completion callback makes the timestamp write conditional on a live actor or hold
(lib.rs:1518–1534, environment/lifecycle.rs:101–111). The callback then unconditionally writes the earlier copied stamp
back to the same row (tensorlake/lifecycle.rs:152–177 and store.rs:7239–7258). If that short blocking write starts
before runtime drop, teardown allows it to finish (lib.rs:1332–1355. Supervisor/db.rs:229–243 and pinned Tokio 1.53.2
runtime/runtime.rs:30–44). This requires quit to overlap the operation between the prior clear and the callback. On
reopening while the sandbox is suspended, the new actor's watermark starts empty (manager.rs:2159–2167,4365–4388) and
the old attribution can be shown. SPEC.md:340–347 and SPEC_impl.md:6723–6727,6756–6767,6777–6781 govern attribution.
Possible_cover: the accepted missed-observation and failed-clear limits, neither of which covers restoring a stamp after
a Running observation and a committed clear. Distinct from f4, f5, f11 and f12. Estimated bucket other, wording only. No
relevant changes through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tensorlake/f13`.
