use std::collections::HashMap;
use std::env;
use std::time::Duration;

use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.demo.vlam.ai/v2.1/projects/poc/openai-compatible/v1";
const DEFAULT_MODEL: &str = "vlam/ubiops-deployment/bzk-dig-mistralmedium-flexibel//chat-model";

const SYSTEM_PROMPT: &str = "\
Je bent een assistent die operatiebomen van Nederlandse wetgeving beschrijft.\n\
\n\
Je ontvangt een boom van operaties die een berekening of beslissing uit een wet \
implementeren. Elke operatie heeft een nummer (bijv. \"1\", \"1.1\", \"1.1.1\") \
en een type (ADD, SUBTRACT, IF, EQUALS, AND, etc.).\n\
\n\
Genereer voor elke operatie een korte, beschrijvende titel in het Nederlands die \
uitlegt wat de operatie doet in de context van de berekening. De titel moet:\n\
- Maximaal 8 woorden zijn\n\
- In het Nederlands geschreven zijn\n\
- De juridische/rekenkundige betekenis beschrijven, niet de technische operatie\n\
- Begrijpelijk zijn voor een jurist zonder technische achtergrond\n\
\n\
Antwoord uitsluitend met een JSON-object waarbij de sleutels de operatienummers \
zijn en de waarden de titels. Geen extra tekst.\n\
\n\
Voorbeeld:\n\
{\"1\": \"Bereken hoogte zorgtoeslag\", \"1.1\": \"Bepaal standaardpremie\", \
\"1.2\": \"Trek normpremie af\"}";

pub struct VlamConfig {
    api_key: String,
    base_url: String,
    model: String,
}

impl VlamConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = env::var("VLAM_API_KEY").ok().filter(|k| !k.is_empty())?;
        let base_url = env::var("VLAM_BASE_URL")
            .ok()
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let model = env::var("VLAM_MODEL")
            .ok()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Some(Self {
            api_key,
            base_url,
            model,
        })
    }
}

#[derive(Clone)]
pub struct VlamClient {
    http: reqwest::Client,
    endpoint: String,
    model: String,
}

impl VlamClient {
    pub fn new(config: VlamConfig) -> Result<Self, VlamError> {
        let mut headers = reqwest::header::HeaderMap::new();
        let auth_value = format!("Bearer {}", config.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            auth_value
                .parse()
                .map_err(|_| VlamError::RequestFailed("invalid API key characters".to_string()))?,
        );

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|e| VlamError::RequestFailed(e.to_string()))?;

        let endpoint = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

        Ok(Self {
            http,
            endpoint,
            model: config.model,
        })
    }

    pub async fn generate_titles(
        &self,
        operations: &Value,
    ) -> Result<HashMap<String, String>, VlamError> {
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": operations.to_string() },
            ],
            "temperature": 0.3,
        });

        let resp = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| VlamError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(VlamError::RequestFailed(format!(
                "VLAM returned {status}: {text}"
            )));
        }

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| VlamError::ParseFailed(e.to_string()))?;

        let content = resp_json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                VlamError::ParseFailed("missing choices[0].message.content".to_string())
            })?;

        // The LLM may wrap the JSON in markdown code fences; strip them.
        let cleaned = content
            .trim()
            .strip_prefix("```json")
            .or_else(|| content.trim().strip_prefix("```"))
            .unwrap_or(content.trim());
        let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();

        let titles: HashMap<String, String> = serde_json::from_str(cleaned)
            .map_err(|e| VlamError::ParseFailed(format!("invalid JSON from LLM: {e}")))?;

        Ok(titles)
    }
}

#[derive(Debug)]
pub enum VlamError {
    NotConfigured,
    RequestFailed(String),
    ParseFailed(String),
}

impl std::fmt::Display for VlamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "AI title generation not configured"),
            Self::RequestFailed(msg) => write!(f, "VLAM request failed: {msg}"),
            Self::ParseFailed(msg) => write!(f, "VLAM response parse error: {msg}"),
        }
    }
}
