//! A stand-in for the helm's file transfers in GUI tests: the test receives each stat, download, download of bytes and
//! upload the GUI asked for and answers it when it chooses.
//!
//! The GUI is written against [`Transfers`]; [`FakeTransfers::new`] hands out the same type, so the GUI's file-link
//! and attachment code runs unchanged with no socket, runtime or timer. Each transfer the GUI starts shows up in
//! [`FakeTransfers::take`] in the order it was started, carrying what the real client would have sent, and its
//! [`Transfer`](crate::transfers::Transfer) stays pending until the test answers it. Holding an answer is how a test
//! observes the in-flight state (a file hover that stays silent, an upload strip); answering with a
//! [`TransferError`] is how it stages a refusal or a dead connection.
//!
//! NOTE: Like the other fakes, this models outcomes, not mechanics. It reads no file and makes no HTTP request: an
//! upload records the file path or bytes the GUI handed over, and the test reads that file itself if it cares.
//! The real client's own tests cover the wire.

use tokio::sync::mpsc;

use crate::transfers::{
    FileStat, Reply, Request, SavedDownload, TransferError, Transfers, UploadSource,
    request_channel,
};

/// The test's side of the GUI's transfers.
pub struct FakeTransfers {
    requests: mpsc::UnboundedReceiver<Request>,
}

/// One transfer the GUI started, with what it asked for and the answer it waits on.
#[derive(Debug)]
pub enum FakeTransfer {
    Stat {
        session: String,
        path: String,
        answer: FakeAnswer<FileStat>,
    },
    Download {
        session: String,
        path: String,
        open: bool,
        answer: FakeAnswer<SavedDownload>,
    },
    /// A read of a file's contents into memory ([`Transfers::download_bytes`]). The fake enforces no cap: the test
    /// answers with whatever bytes or [`TransferError::TooLarge`] its case needs.
    DownloadBytes {
        session: String,
        path: String,
        /// The cap the GUI asked the real client to enforce.
        max_bytes: usize,
        answer: FakeAnswer<Vec<u8>>,
    },
    Upload {
        session: String,
        /// The local file or the bytes the GUI asked to attach.
        source: UploadSource,
        /// The name the GUI proposed for it on the host.
        filename: String,
        answer: FakeAnswer<String>,
    },
}

/// The pending answer to one transfer. Keep it to hold the transfer in flight; drop it unanswered and the GUI sees
/// [`TransferError::Lost`], as when the real client's runtime stops.
#[derive(Debug)]
pub struct FakeAnswer<T>(Reply<T>);

impl<T> FakeAnswer<T> {
    /// Resolve the GUI's future with `result`.
    pub fn answer(self, result: Result<T, TransferError>) {
        let _ = self.0.send(result);
    }

    /// Whether the GUI dropped its future, which on the real client abandons the request and closes its connection.
    /// This is what a test checks to see that leaving a link, say, cancelled its check.
    pub fn abandoned(&self) -> bool {
        self.0.is_closed()
    }
}

impl FakeTransfers {
    /// A fake with the handle the GUI under test should get.
    pub fn new() -> (FakeTransfers, Transfers) {
        let (transfers, requests) = request_channel();
        (FakeTransfers { requests }, transfers)
    }

    /// The transfers the GUI started since the last call, in order.
    pub fn take(&mut self) -> Vec<FakeTransfer> {
        let mut taken = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            taken.push(match request {
                Request::Stat {
                    session,
                    path,
                    reply,
                } => FakeTransfer::Stat {
                    session,
                    path,
                    answer: FakeAnswer(reply),
                },
                Request::Download {
                    session,
                    path,
                    open,
                    reply,
                } => FakeTransfer::Download {
                    session,
                    path,
                    open,
                    answer: FakeAnswer(reply),
                },
                Request::DownloadBytes {
                    session,
                    path,
                    max_bytes,
                    reply,
                } => FakeTransfer::DownloadBytes {
                    session,
                    path,
                    max_bytes,
                    answer: FakeAnswer(reply),
                },
                Request::Upload {
                    session,
                    source,
                    filename,
                    reply,
                } => FakeTransfer::Upload {
                    session,
                    source,
                    filename,
                    answer: FakeAnswer(reply),
                },
            });
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfers::UploadOutcome;
    use farhelm_proto::{DownloadFileInfo, DownloadFileStatus};
    use futures_util::FutureExt;
    use std::path::PathBuf;

    /// GUI tests trust this fake in place of the helm, so it must keep the client's contract with no runtime:
    /// transfers arrive in the order the GUI started them with what it asked for, each stays pending while the test
    /// holds its answer, an answer (a result, a refusal or a lost connection) reaches exactly its own future, and a
    /// future the GUI dropped shows as abandoned.
    #[test]
    fn the_fake_records_each_transfer_and_holds_its_answer_until_the_test_gives_it() {
        let (mut fake, transfers) = FakeTransfers::new();
        let mut stat = transfers.stat("s1", "notes/a.md");
        let mut download = transfers.download("s1", "/tmp/b.pdf", true);
        let mut contents = transfers.download_bytes("s1", "notes/c.txt", 10);
        let mut upload = transfers.upload("s2", PathBuf::from("/local/shot.png"), "shot.png");
        let pasted = transfers.upload_bytes("s2", vec![1, 2, 3], "pasted-1.png");
        let abandoned = transfers.stat("s1", "gone.txt");
        drop(abandoned);

        let mut taken = fake.take().into_iter();
        let Some(FakeTransfer::Stat {
            session,
            path,
            answer: stat_answer,
        }) = taken.next()
        else {
            panic!("the stat came first");
        };
        assert_eq!((session.as_str(), path.as_str()), ("s1", "notes/a.md"));
        let Some(FakeTransfer::Download {
            path,
            open,
            answer: download_answer,
            ..
        }) = taken.next()
        else {
            panic!("the download came second");
        };
        assert_eq!((path.as_str(), open), ("/tmp/b.pdf", true));
        let Some(FakeTransfer::DownloadBytes {
            session,
            path,
            max_bytes,
            answer: contents_answer,
        }) = taken.next()
        else {
            panic!("the download of bytes came third");
        };
        assert_eq!(
            (session.as_str(), path.as_str(), max_bytes),
            ("s1", "notes/c.txt", 10)
        );
        let Some(FakeTransfer::Upload {
            session,
            source,
            filename,
            answer: upload_answer,
        }) = taken.next()
        else {
            panic!("the upload came fourth");
        };
        assert_eq!(
            (session.as_str(), source, filename.as_str()),
            (
                "s2",
                UploadSource::File(PathBuf::from("/local/shot.png")),
                "shot.png"
            )
        );
        let Some(FakeTransfer::Upload {
            source,
            filename,
            answer: _pasted_answer,
            ..
        }) = taken.next()
        else {
            panic!("the upload from memory came fifth");
        };
        assert_eq!(
            (source, filename.as_str()),
            (UploadSource::Bytes(vec![1, 2, 3]), "pasted-1.png")
        );
        assert!(pasted.now_or_never().is_none());
        let Some(FakeTransfer::Stat { answer: gone, .. }) = taken.next() else {
            panic!("the abandoned stat was still recorded");
        };
        assert!(gone.abandoned());
        assert!(taken.next().is_none());

        // Held: nothing resolves until the test answers.
        assert!((&mut stat).now_or_never().is_none());
        assert!((&mut download).now_or_never().is_none());
        assert!((&mut contents).now_or_never().is_none());
        assert!((&mut upload).now_or_never().is_none());
        assert!(!stat_answer.abandoned());

        let checked = FileStat {
            host: "local".into(),
            can_open: Some(true),
            info: DownloadFileInfo {
                path: "/home/u/notes/a.md".into(),
                size: Some(12),
                status: DownloadFileStatus::Ready,
            },
        };
        stat_answer.answer(Ok(checked.clone()));
        assert_eq!(stat.now_or_never(), Some(Ok(checked)));

        let refusal = TransferError::Refused {
            status: 404,
            message: "native Downloads save is unavailable".into(),
        };
        download_answer.answer(Err(refusal.clone()));
        assert_eq!(download.now_or_never(), Some(Err(refusal)));

        contents_answer.answer(Ok(b"hello".to_vec()));
        assert_eq!(contents.now_or_never(), Some(Ok(b"hello".to_vec())));

        upload_answer.answer(Err(TransferError::Lost("connection reset".into())));
        let lost = upload.now_or_never().unwrap().unwrap_err();
        assert_eq!(lost.upload_outcome(), UploadOutcome::Unknown);

        // A dropped answer reads as a lost connection, never as a hang.
        let pending = transfers.download("s1", "c.txt", false);
        drop(fake.take());
        assert!(matches!(
            pending.now_or_never(),
            Some(Err(TransferError::Lost(_)))
        ));
    }
}
