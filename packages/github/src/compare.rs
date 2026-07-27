//! Compare API: the in-repo paths of files that differ between two refs.

use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

#[derive(Debug, Deserialize)]
struct CompareResponse {
    #[serde(default)]
    files: Vec<CompareFile>,
}
#[derive(Debug, Deserialize)]
struct CompareFile {
    filename: String,
}

impl GithubClient {
    /// In-repo paths of files that differ between `base` and `head`, via the
    /// Compare API (`{base}...{head}` — three-dot, so the result is what
    /// `head` introduced since the merge base).
    ///
    /// A 404 (base or head ref missing — e.g. a traject branch never created
    /// because nothing has been saved yet) maps to an empty list: "no branch
    /// yet" is the normal pre-edit state, treated as "nothing changed".
    ///
    /// NOTE: the Compare API caps `files` at 300 and paginates beyond that;
    /// this reads the first page only (a realistic traject edits a handful of
    /// laws).
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "compare", repo = %repo))]
    pub async fn compare_files(
        &self,
        repo: &str,
        base: &str,
        head: &str,
        token: Option<&str>,
    ) -> Result<Vec<String>> {
        let url = format!(
            "{}/repos/{}/compare/{}...{}",
            self.api_base, repo, base, head
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Compare API {base}...{head} on {repo}: {body}"),
            });
        }
        let parsed: CompareResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse compare response: {e}")))?;
        Ok(parsed.files.into_iter().map(|f| f.filename).collect())
    }
}
