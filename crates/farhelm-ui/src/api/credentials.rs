//! Explicit credential writes outlive their editor; reads return metadata only.

use super::{client, encode_path_segment, read_failure, refusal_text, send_read, send_save};
use farhelm_proto::credentials::{CreateCredential, Credential, SaveCredential, SavedCredential};

/// Keep a failed read distinct from an empty catalog so the editor can offer Retry.
pub(crate) async fn fetch_credentials(base: &str) -> Result<Vec<Credential>, String> {
    let url = format!("{base}/api/credentials");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response
        .json()
        .await
        .map_err(|_| "Could not read credentials".to_string())
}

/// Register the secret-bearing request before returning its owned future.
/// A replacement deliberately sends neither the fixed name nor the fixed kind.
/// Normalize create's metadata response to the replacement acknowledgement so
/// an editor handles a saved value separately from any delivery failures.
pub(crate) fn write_credential(
    base: &str,
    replacing: bool,
    draft: CreateCredential,
) -> impl Future<Output = Result<SavedCredential, String>> + use<> {
    let (method, url, request) = if replacing {
        let url = format!(
            "{base}/api/credentials/{}",
            encode_path_segment(&draft.name)
        );
        let request = client().put(&url).json(&SaveCredential {
            secret: Some(draft.secret),
        });
        ("PUT", url, request)
    } else {
        let url = format!("{base}/api/credentials");
        let request = client().post(&url).json(&draft);
        ("POST", url, request)
    };
    let sending = send_save(request);
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text(method, &url, response).await);
        }
        if replacing {
            response
                .json()
                .await
                .map_err(|_| "Could not read the saved credential".to_string())
        } else {
            response
                .json::<Credential>()
                .await
                .map(|credential| SavedCredential {
                    credential,
                    delivery_failures: Vec::new(),
                })
                .map_err(|_| "Could not read the saved credential".to_string())
        }
    }
}

/// Deletion is an explicit save too, including when the editor closes mid-request.
pub(crate) fn delete_credential(
    base: &str,
    name: &str,
) -> impl Future<Output = Result<(), String>> + use<> {
    let url = format!("{base}/api/credentials/{}", encode_path_segment(name));
    let sending = send_save(client().delete(&url));
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("DELETE", &url, response).await);
        }
        Ok(())
    }
}
