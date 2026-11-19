//! The local rendezvous between OpenSSH's askpass child and the helm's pending sign-in list.
//!
//! ssh receives the answer on its helper's stdout. The helm never places an answer in an argument, environment
//! variable, event, persisted record or diagnostic. An attempt owns its private socket and removes its question when
//! the helper exits. A question id is valid for one prompt, even when ssh asks several questions on one connection.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as BlockingStream;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Bytes;
use axum::extract::{Path as ApiPath, State};
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};
use farhelm_proto::ui_channel::SshQuestion;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::AppState;
use crate::feed::FleetEvents;

/// OpenSSH calls the executable with just its question. The private symlink gives the CLI an unambiguous entry point.
pub const ASKPASS_PROGRAM_NAME: &str = "farhelm-askpass";
/// OpenSSH uses this slash-free program name when it generates the command for a jump host.
pub const ASKPASS_JUMP_PROGRAM_NAME: &str = "farhelm-ssh-jump";
pub(crate) const ASKPASS_SOCKET_ENV: &str = "FARHELM_SSH_ASKPASS_SOCKET";
pub(crate) const ASKPASS_SSH_PROGRAM_ENV: &str = "FARHELM_SSH_ASKPASS_PROGRAM";

const MAX_QUESTION_BYTES: usize = 2048;
/// OpenSSH 9.6 reads up to 1023 bytes and cuts the answer at a line break. Refuse input it would silently change.
const MAX_ANSWER_BYTES: usize = 1023;
const QUESTION: u8 = 0;
const NOTICE: u8 = 1;

/// A response only lives in the pending receiver or in the helper's pipe. It cannot be debug-printed.
pub(crate) enum Answer {
    Send(String),
    Cancel,
}

struct Pending {
    view: SshQuestion,
    answer: oneshot::Sender<Answer>,
    deadline: Instant,
}

/// Each helm owns one pending list. Its short-lived entries never hold the registry mutex across network I/O.
#[derive(Default)]
pub(crate) struct Questions {
    pending: Mutex<BTreeMap<String, Pending>>,
    events: OnceLock<Arc<FleetEvents>>,
}

impl Questions {
    /// The list can receive a question during the manager's startup. The first UI snapshot reads the current list.
    pub(crate) fn publish_to(&self, events: Arc<FleetEvents>) {
        let _ = self.events.set(events);
    }

    fn changed(&self) {
        if let Some(events) = self.events.get() {
            events.bump();
        }
    }

    pub(crate) fn list(&self) -> Vec<SshQuestion> {
        let pending = self.pending.lock().expect("SSH questions mutex");
        let mut shown = pending
            .values()
            .map(|entry| entry.view.clone())
            .collect::<Vec<_>>();
        shown.sort_by(|a, b| (a.expires_at, &a.id).cmp(&(b.expires_at, &b.id)));
        shown
    }

    /// The lease clears the question when the caller is interrupted, even if no reply ever reaches ssh.
    pub(crate) fn open(&self, view: SshQuestion, deadline: Instant) -> QuestionLease<'_> {
        let id = view.id.clone();
        let (answer, reply) = oneshot::channel();
        self.pending.lock().expect("SSH questions mutex").insert(
            id.clone(),
            Pending {
                view,
                answer,
                deadline,
            },
        );
        self.changed();
        QuestionLease {
            questions: self,
            id,
            reply,
        }
    }

    fn close(&self, id: &str) {
        let removed = self.pending.lock().expect("SSH questions mutex").remove(id);
        if removed.is_some() {
            self.changed();
        }
    }

    pub(crate) fn reply(&self, id: &str, answer: Answer) -> StatusCode {
        let entry = {
            let mut pending = self.pending.lock().expect("SSH questions mutex");
            let Some(entry) = pending.get(id) else {
                return StatusCode::NOT_FOUND;
            };
            if entry.deadline <= Instant::now() {
                return StatusCode::NOT_FOUND;
            }
            if entry.view.notice && matches!(answer, Answer::Send(_)) {
                return StatusCode::UNPROCESSABLE_ENTITY;
            }
            pending
                .remove(id)
                .expect("same question under the same lock")
        };
        let delivered = entry.answer.send(answer).is_ok();
        self.changed();
        if delivered {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
    }
}

/// A question and its one reply. Dropping the attempt revokes the reply id before a later attempt can be shown.
pub(crate) struct QuestionLease<'a> {
    questions: &'a Questions,
    id: String,
    pub(crate) reply: oneshot::Receiver<Answer>,
}

impl Drop for QuestionLease<'_> {
    fn drop(&mut self) {
        self.questions.close(&self.id);
    }
}

pub(crate) async fn list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({ "questions": state.ssh_questions.list() }))
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum Reply {
    Send { answer: String },
    Cancel,
}

/// Read a bounded raw body and give every malformed reply the same error, including if it contains an answer of the
/// wrong JSON type. Serde's rejection text can include user input.
pub(crate) async fn reply(
    State(state): State<Arc<AppState>>,
    ApiPath(id): ApiPath<String>,
    body: Bytes,
) -> StatusCode {
    let answer = match serde_json::from_slice::<Reply>(&body) {
        Ok(Reply::Send { answer }) if valid_answer(&answer) => Answer::Send(answer),
        Ok(Reply::Cancel) => Answer::Cancel,
        _ => return StatusCode::UNPROCESSABLE_ENTITY,
    };
    state.ssh_questions.reply(&id, answer)
}

fn valid_answer(answer: &str) -> bool {
    answer.len() <= MAX_ANSWER_BYTES && !answer.contains(['\0', '\r', '\n'])
}

/// The helper holds its end of the stream open while the question is visible. Preserve line breaks in a fingerprint
/// or verification menu. The presentation boundary escapes the other control characters.
pub(crate) async fn read_question(stream: &mut UnixStream) -> std::io::Result<(String, bool)> {
    let kind = stream.read_u8().await?;
    let length = usize::from(stream.read_u16().await?);
    if !matches!(kind, QUESTION | NOTICE) || length > MAX_QUESTION_BYTES {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), kind == NOTICE))
}

/// OpenSSH accepts a byte stream ending at EOF as the answer. Use a length prefix only between helm and helper, so
/// an interrupted relay can never look like a completed empty answer.
pub(crate) async fn send_answer(stream: &mut UnixStream, answer: String) -> std::io::Result<()> {
    stream.write_u16(answer.len() as u16).await?;
    stream.write_all(answer.as_bytes()).await
}

/// Entry point for the CLI when OpenSSH executes the private helper symlink. No failure prints the question or answer.
pub fn run_askpass(question: &std::ffi::OsStr) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;

    let socket = std::env::var_os(ASKPASS_SOCKET_ENV).ok_or(std::io::ErrorKind::NotFound)?;
    let notice = std::env::var_os("SSH_ASKPASS_PROMPT").is_some_and(|hint| hint == "none");
    let mut stream = BlockingStream::connect(Path::new(&socket))?;
    let question = question.as_bytes();
    let end = question.len().min(MAX_QUESTION_BYTES);
    stream.write_all(&[if notice { NOTICE } else { QUESTION }])?;
    stream.write_all(&(end as u16).to_be_bytes())?;
    stream.write_all(&question[..end])?;
    let mut length = [0; 2];
    stream.read_exact(&mut length)?;
    let length = usize::from(u16::from_be_bytes(length));
    if notice || length > MAX_ANSWER_BYTES {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut answer = vec![0; length];
    stream.read_exact(&mut answer)?;
    std::io::stdout().lock().write_all(&answer)
}

/// The generated ProxyJump command omits the parent client's `-o` flags. Supply the same one-try limit to each hop,
/// with the original argv forwarded intact. The program name comes from the helm-owned launch environment.
pub fn run_askpass_jump(args: impl IntoIterator<Item = std::ffi::OsString>) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt as _;

    let program = std::env::var_os(ASKPASS_SSH_PROGRAM_ENV).ok_or(std::io::ErrorKind::NotFound)?;
    Err(std::process::Command::new(program)
        .arg0(ASKPASS_JUMP_PROGRAM_NAME)
        .args(["-o", crate::ssh::SSH_ONE_PASSWORD_PROMPT])
        .args(args)
        .exec())
}
