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

// Caller-supplied strings get truncated before they reach the LLM so a hostile
// payload can't burn unbounded API budget or smuggle a wall of "ignore previous
// instructions" text into the prompt. Numbers picked to fit comfortably around
// real zorgtoeslag traces (~10 KB) with headroom.
const MAX_IDENTIFIER_CHARS: usize = 200;
const MAX_PROFILE_SUMMARY_CHARS: usize = 500;
const MAX_PARAMETERS_CHARS: usize = 4_000;
const MAX_RESULT_CHARS: usize = 16_000;
const MAX_TRACE_CHARS: usize = 32_000;

fn clip(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!(
        "{truncated}… [{} tekens afgekapt]",
        s.chars().count() - max_chars
    )
}

pub fn build_user_prompt(req: &ExplainRequest) -> String {
    let mut out = String::new();
    out.push_str("Wet: ");
    if req.law_label.is_empty() {
        out.push_str(&clip(&req.law_id, MAX_IDENTIFIER_CHARS));
    } else {
        out.push_str(&clip(&req.law_label, MAX_IDENTIFIER_CHARS));
    }
    out.push('\n');
    out.push_str("Gevraagde uitkomst: ");
    out.push_str(&clip(&req.output_name, MAX_IDENTIFIER_CHARS));
    out.push_str("\n\n");

    if !req.profile_summary.is_empty() {
        out.push_str("Over de burger:\n");
        out.push_str(&clip(&req.profile_summary, MAX_PROFILE_SUMMARY_CHARS));
        out.push_str("\n\n");
    }

    out.push_str("Invoer-parameters (JSON):\n");
    out.push_str(&clip(&req.parameters.to_string(), MAX_PARAMETERS_CHARS));
    out.push_str("\n\n");

    out.push_str("Uitvoerings-resultaat (JSON):\n");
    out.push_str(&clip(&req.result.to_string(), MAX_RESULT_CHARS));
    out.push_str("\n\n");

    if !req.trace.is_null() {
        out.push_str("Trace (JSON):\n");
        out.push_str(&clip(&req.trace.to_string(), MAX_TRACE_CHARS));
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

    // Send the system prompt as a content-block array with `cache_control:
    // ephemeral` so Anthropic caches the prefix across calls. Reduces first-token
    // latency on cache hits at zero risk: cache misses fall back to a normal
    // prompt and the system text is still applied.
    let body = serde_json::json!({
        "model": config.model,
        "max_tokens": 512,
        "system": [
            {
                "type": "text",
                "text": SYSTEM_PROMPT,
                "cache_control": { "type": "ephemeral" }
            }
        ],
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
        let text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable error body>".to_string());
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
    fn long_user_input_gets_clipped() {
        let big = "x".repeat(50_000);
        let req = ExplainRequest {
            law_id: "z".into(),
            law_label: String::new(),
            output_name: "y".into(),
            parameters: json!({}),
            result: json!({}),
            trace: Value::Null,
            profile_summary: big,
        };
        let p = build_user_prompt(&req);
        assert!(p.contains("tekens afgekapt"));
        assert!(p.len() < 10_000);
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
