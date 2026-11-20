//! The real local process and socket lifetime, with the remote side of SSH replaced by a pipe that stays open.

use super::*;
use std::os::unix::process::ExitStatusExt as _;
use tokio::io::AsyncWriteExt as _;
use tokio::time::timeout;

const TEST_WAIT: Duration = Duration::from_secs(5);

async fn split_question(stream: &mut UnixStream) {
    stream.write_u8(0).await.unwrap();
    stream.write_u16(5).await.unwrap();
    stream.write_all(b"Code:").await.unwrap();
}

/// Cancel must kill SSH while its helper still waits for an answer. An EOF first can become an empty 2FA reply.
#[farhelm_testtrace::test]
async fn cancellation_kills_the_owned_process_and_revokes_the_question() {
    let dir = tempfile::Builder::new()
        .prefix("fh-askpass-")
        .tempdir_in("/tmp")
        .unwrap();
    let path = dir.path().join("ask");
    let listener = UnixListener::bind(&path).unwrap();
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let _keep_pipe_open = child.stdin.take().unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "the counterpart must be alive when the prompt is installed"
    );
    let mut process = InteractiveChild::new(child);
    let questions = Questions::default();
    let events = Arc::new(crate::feed::FleetEvents::new());
    let mut changed = events.subscribe();
    questions.publish_to(events);

    timeout(TEST_WAIT, async {
        let ((), ()) = tokio::join!(
            async {
                let result = process
                    .run(
                        listener,
                        &questions,
                        17,
                        true,
                        Instant::now() + TEST_WAIT,
                        &mut |_| {},
                    )
                    .await;
                assert!(result.unwrap_err().is::<SignInNeeded>());
                assert_eq!(
                    process.cancel().await.unwrap().signal(),
                    Some(libc::SIGKILL)
                );
                assert!(
                    process.helper.is_some(),
                    "the helper socket must stay open through reaping"
                );
                drop(process);
            },
            async {
                // A touch can finish while its notice helper writes the frame. The live client can still ask a
                // subsequent question.
                let mut notice = UnixStream::connect(&path).await.unwrap();
                notice.write_u8(1).await.unwrap();
                drop(notice);
                let mut helper = UnixStream::connect(&path).await.unwrap();
                split_question(&mut helper).await;
                changed.changed().await.unwrap();
                let shown = questions.list();
                assert_eq!(shown.len(), 1);
                assert_eq!(shown[0].prompt, "Code:");
                assert!(
                    shown[0].startup,
                    "the first-dial origin travels with the question"
                );
                assert_eq!(
                    questions.reply(&shown[0].id, Answer::Cancel),
                    axum::http::StatusCode::NO_CONTENT
                );
                assert_eq!(
                    helper.read_u8().await.unwrap_err().kind(),
                    std::io::ErrorKind::UnexpectedEof
                );
                assert!(questions.list().is_empty());
            },
        );
    })
    .await
    .expect(
        "the owned sign-in should end on cancellation without waiting for the authentication timer",
    );
}

/// A silent SSH peer reaches the ordinary dial deadline. A local kill must not masquerade as SSH's exit status
/// or put the host in the state that requires the user to sign in again.
#[farhelm_testtrace::test]
async fn a_silent_peer_expires_without_a_sign_in_question() {
    let dir = tempfile::tempdir_in("/tmp").unwrap();
    let listener = UnixListener::bind(dir.path().join("ask")).unwrap();
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let _keep_pipe_open = child.stdin.take().unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "premise: the child is live"
    );
    let mut process = InteractiveChild::new(child);
    let questions = Questions::default();
    let error = process
        .run(listener, &questions, 17, false, Instant::now(), &mut |_| {
            panic!("the silent peer should never publish a question");
        })
        .await
        .expect_err("the unanswered dial reached its deadline");
    let failure = error
        .downcast_ref::<BeforeQuestion>()
        .expect("ordinary dial failure");
    assert!(failure.timed_out);
    assert!(failure.snapshot.exit.is_none());
    assert!(questions.list().is_empty());
    assert_eq!(
        process.cancel().await.unwrap().signal(),
        Some(libc::SIGKILL)
    );
}
