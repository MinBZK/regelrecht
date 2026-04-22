//! Prompt construction + Anthropic API call.
//!
//! The explainer is kept small on purpose: single model, single message,
//! no streaming. If we want streaming to the browser, that becomes another
//! layer above this module.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct ExplainerConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Deserialize, Debug)]
pub struct ExplainRequest {
    pub law_id: String,
    #[serde(default)]
    pub law_label: String,
    pub output_name: String,
    #[serde(default)]
    pub parameters: Value,
    pub result: Value,
    #[serde(default)]
    pub trace: Value,
    #[serde(default)]
    pub profile_summary: String,
}

#[derive(Serialize, Debug)]
pub struct ExplainResponse {
    pub explanation: String,
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("upstream rate-limited us")]
    UpstreamRateLimit,
    #[error("upstream returned unexpected payload: {0}")]
    BadResponse(String),
    #[error("http client error: {0}")]
    Http(#[from] reqwest::Error),
}

impl ExplainError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ExplainError::UpstreamRateLimit => StatusCode::TOO_MANY_REQUESTS,
            ExplainError::Upstream(_) | ExplainError::BadResponse(_) => StatusCode::BAD_GATEWAY,
            ExplainError::Http(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

const SYSTEM_PROMPT: &str = "Je bent een Nederlandse ambtenaar die burgers helpt begrijpen welke regels op hen van toepassing zijn. \
Je geeft een korte, begrijpelijke uitleg in het Nederlands (B1-niveau). \
Leg stap voor stap uit waarom de uitkomst is wat hij is, gebaseerd op de aangeleverde trace. \
Verzin geen getallen of regels; gebruik alleen wat in de invoer of trace staat. \
Eindig met één zin die de burger vertelt wat hij hiermee kan doen.";

pub fn build_user_prompt(req: &ExplainRequest) -> String {
    let mut out = String::new();
    out.push_str("Wet: ");
    if req.law_label.is_empty() {
        out.push_str(&req.law_id);
    } else {
        out.push_str(&req.law_label);
    }
    out.push('\n');
    out.push_str("Gevraagde uitkomst: ");
    out.push_str(&req.output_name);
    out.push_str("\n\n");

    if !req.profile_summary.is_empty() {
        out.push_str("Over de burger:\n");
        out.push_str(&req.profile_summary);
        out.push_str("\n\n");
    }

    out.push_str("Invoer-parameters (JSON):\n");
    out.push_str(&req.parameters.to_string());
    out.push_str("\n\n");

    out.push_str("Uitvoerings-resultaat (JSON):\n");
    out.push_str(&req.result.to_string());
    out.push_str("\n\n");

    if !req.trace.is_null() {
        out.push_str("Trace (JSON):\n");
        out.push_str(&req.trace.to_string());
        out.push_str("\n\n");
    }

    out.push_str(
        "Leg uit in 4–6 zinnen. Gebruik getallen alleen als ze in de invoer of trace staan.",
    );
    out
}

pub async fn explain(
    http: &reqwest::Client,
    config: &ExplainerConfig,
    req: ExplainRequest,
) -> Result<ExplainResponse, ExplainError> {
    let user_prompt = build_user_prompt(&req);

    let body = serde_json::json!({
        "model": config.model,
        "max_tokens": 512,
        "system": SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": user_prompt }
        ]
    });

    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ExplainError::UpstreamRateLimit);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ExplainError::Upstream(format!("{status}: {text}")));
    }

    let payload: Value = resp.json().await?;
    let text = payload
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .find_map(|item| item.get("text").and_then(|t| t.as_str()))
        })
        .ok_or_else(|| ExplainError::BadResponse(payload.to_string()))?
        .to_string();

    Ok(ExplainResponse {
        explanation: text,
        model: config.model.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_prompt_includes_core_fields() {
        let req = ExplainRequest {
            law_id: "zorgtoeslagwet".into(),
            law_label: "Zorgtoeslag".into(),
            output_name: "hoogte_zorgtoeslag".into(),
            parameters: json!({ "bsn": "100000001" }),
            result: json!({ "outputs": { "hoogte_zorgtoeslag": 12500 } }),
            trace: json!({ "steps": ["inkomen < drempel"] }),
            profile_summary: "Alleenstaande ZZP'er".into(),
        };
        let p = build_user_prompt(&req);
        assert!(p.contains("Zorgtoeslag"));
        assert!(p.contains("hoogte_zorgtoeslag"));
        assert!(p.contains("Alleenstaande ZZP'er"));
        assert!(p.contains("100000001"));
        assert!(p.contains("inkomen < drempel"));
    }

    #[test]
    fn falls_back_to_law_id_when_label_missing() {
        let req = ExplainRequest {
            law_id: "zorgtoeslagwet".into(),
            law_label: String::new(),
            output_name: "x".into(),
            parameters: json!({}),
            result: json!({}),
            trace: Value::Null,
            profile_summary: String::new(),
        };
        let p = build_user_prompt(&req);
        assert!(p.contains("Wet: zorgtoeslagwet"));
        assert!(!p.contains("Trace (JSON)"));
    }

    #[test]
    fn error_statuses_map_correctly() {
        assert_eq!(
            ExplainError::UpstreamRateLimit.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            ExplainError::Upstream("x".into()).status_code(),
            StatusCode::BAD_GATEWAY
        );
    }
}
