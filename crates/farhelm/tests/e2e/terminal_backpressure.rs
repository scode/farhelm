//! Measure the two processes that retain terminal state while a real
//! terminal host keeps reading a producer and the client stops viewing it.

mod rss;

use crate::harness::*;
use farhelm_supervisor::hosts::{DEFAULT_JOURNAL_CAP, HostTerminal, Liveness};

/// Sample just after an accepted checkpoint. The journal may grow between
/// checkpoints even at steady state, so compare the same part of each cycle.
async fn wait_for_checkpoint(host: &HostTerminal, minimum: u64) {
    let mut status = host.subscribe();
    tokio::time::timeout(
        Duration::from_secs(40),
        status.wait_for(|status| {
            status
                .checkpoint
                .is_some_and(|checkpoint| checkpoint.offset >= minimum)
        }),
    )
    .await
    .unwrap_or_else(|_| {
        let status = host.status();
        panic!(
            "host liveness {:?}, position {:?}, checkpoint {:?}; never checkpointed {minimum}",
            status.liveness, status.position, status.checkpoint
        )
    })
    .expect("host runtime stays alive while producing checkpoints");
}

/// A busy terminal must keep its host and supervisor memory bounded with a
/// stalled viewer. Warm the scrollback, then start sampling after a
/// journal's worth of new output and an accepted checkpoint. The extra
/// output lets the emulator and host reuse storage before the baseline.
///
/// The fixture's progress file and the runtime's journal position show
/// actual output while the viewer is paused. Both exact process identities
/// are sampled under nextest's all-slot reservation.
#[farhelm_testtrace::test]
async fn memory_stays_flat_while_a_viewer_is_stalled() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("working directory");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script flood-memory"),
            None,
            80,
            24,
        )
        .await
        .expect("create a session with a gated producer");
    let host = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("real host");
    let process = host.adopted_process().expect("adopted host process");
    let host_memory = rss::ProcessRss::bind(process.pid).expect("bind host RSS");
    assert_eq!(host_memory.start_time_ticks, process.start);
    let own_memory = rss::ProcessRss::bind(std::process::id()).expect("bind supervisor RSS");

    let (channel, mut seen, mut stream) = h.client.attach_live(&session.id, 80, 24).await.unwrap();
    wait_for(&mut stream, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_keys(channel, b"w".to_vec()).await;
    wait_for(&mut stream, &mut seen, "FLOOD-WARMED", 45).await;
    drop(seen);
    h.client.pause_output(channel).await;
    assert!(
        drain_for(&mut stream, &mut Vec::new(), Duration::from_secs(1))
            .await
            .is_none(),
        "the viewer must remain attached when paused"
    );

    let journal_target = host.status().position.offset + DEFAULT_JOURNAL_CAP;
    h.client.send_keys(channel, b"m".to_vec()).await;
    wait_for_file(&work.path().join("flood-memory-started"), 20).await;
    let progress = work.path().join("flood-memory-progress");
    wait_for_file(&progress, 20).await;
    wait_for_checkpoint(&host, journal_target).await;
    assert!(matches!(host.liveness(), Liveness::Running { .. }));

    let records = || -> u64 {
        std::fs::read_to_string(&progress)
            .expect("read producer progress")
            .parse()
            .expect("progress contains a record count")
    };
    let sampling_started = std::time::Instant::now();
    let sample = |index| {
        let records = records();
        let status = host.status();
        let offset = status.position.offset;
        let host_rss = host_memory.bytes().expect("terminal host RSS");
        let supervisor_rss = own_memory.bytes().expect("supervisor RSS");
        let record = serde_json::json!({
            "sample": index, "elapsed_ms": sampling_started.elapsed().as_millis(),
            "records": records, "journal_offset": offset,
            "checkpoint_offset": status.checkpoint.map(|checkpoint| checkpoint.offset),
            "host_pid": host_memory.pid, "host_start_ticks": host_memory.start_time_ticks,
            "supervisor_pid": own_memory.pid, "supervisor_start_ticks": own_memory.start_time_ticks,
            "host_rss_bytes": host_rss, "supervisor_rss_bytes": supervisor_rss,
            "metric": "smaps_rollup.Rss",
        });
        tracing::info!(sample = %record, "stalled viewer RSS sample");
        eprintln!("RSS_SAMPLE {record}");
        (records, offset, host_rss, supervisor_rss)
    };
    let (initial_records, initial_offset, host_baseline, supervisor_baseline) = sample(0);
    let (mut host_peak, mut supervisor_peak) = (host_baseline, supervisor_baseline);
    let mut sample_offset = initial_offset;
    for index in 1..=6 {
        // Require two more MiB of output per sample so a slow scheduler
        // cannot turn several observations into the same working set.
        wait_for_checkpoint(&host, sample_offset + 2 * 1024 * 1024).await;
        let (_, offset, host_rss, supervisor_rss) = sample(index);
        sample_offset = offset;
        host_peak = host_peak.max(host_rss);
        supervisor_peak = supervisor_peak.max(supervisor_rss);
    }
    let final_records = records();
    let final_offset = host.status().position.offset;
    assert!(
        final_records > initial_records && final_offset >= initial_offset + 8 * 1024 * 1024,
        "not enough output during sampling: producer {initial_records} to {final_records} records, \
         runtime {initial_offset} to {final_offset} bytes"
    );
    const GROWTH_LIMIT: u64 = 8 * 1024 * 1024;
    let host_growth = host_peak - host_baseline;
    let supervisor_growth = supervisor_peak - supervisor_baseline;
    assert!(
        host_growth < GROWTH_LIMIT,
        "terminal host RSS grew by {host_growth} bytes"
    );
    assert!(
        supervisor_growth < GROWTH_LIMIT,
        "supervisor RSS grew by {supervisor_growth} bytes"
    );

    // The client has not drained this stream since the producer started.
    // A held pause leaves no buffered output despite the journal's advance.
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.recv())
            .await
            .is_err(),
        "the viewer received output or detached while it was paused"
    );
    h.client.resume_output(channel).await;
    let resumed = tokio::time::timeout(Duration::from_secs(15), stream.recv()).await;
    assert!(
        matches!(resumed, Ok(Some(TermEvent::Data(bytes))) if !bytes.is_empty()),
        "the same viewer must receive output again after resuming"
    );

    // The producer's out-of-band acknowledgement distinguishes our stop
    // gate from its expiry without draining the remaining output backlog.
    h.client.send_keys(channel, b"d".to_vec()).await;
    let stopped = work.path().join("flood-memory-stopped");
    wait_for_file(&stopped, 10).await;
    assert_eq!(std::fs::read_to_string(stopped).unwrap(), "released");
}
