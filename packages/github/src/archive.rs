//! Archive (tarball) API: download a whole repo at a ref in one request.
//!
//! Returns the raw gzipped-tar bytes; gzip/tar extraction stays in the caller
//! (the corpus domain layer parses each law's `implements` and discards bodies
//! one at a time, so a large corpus archive never materialises in full).

use bytes::Bytes;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

impl GithubClient {
    /// Download the repo at `git_ref` via the tarball endpoint
    /// (`GET /repos/{repo}/tarball/{ref}`) and return the raw response bytes.
    ///
    /// GitHub answers with a 302 to a short-lived codeload URL (carrying its
    /// own token), which reqwest follows automatically — so this works for
    /// private repos with just the Bearer token on the initial request.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "tarball", repo = %repo))]
    pub async fn fetch_tarball(
        &self,
        repo: &str,
        git_ref: &str,
        token: Option<&str>,
    ) -> Result<Bytes> {
        let url = format!("{}/repos/{}/tarball/{}", self.api_base, repo, git_ref);
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub archive request failed: {e}")))?;
        self.track_rate_limit(&response);

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("archive API for {repo}@{git_ref}: {body}"),
            });
        }

        response
            .bytes()
            .await
            .map_err(|e| GithubError::Transport(format!("failed to read archive body: {e}")))
    }
}
