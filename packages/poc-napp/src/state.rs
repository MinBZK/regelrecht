//! Application state shared across handlers.

use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_auth::{ConfiguredClient, OidcAppState, OidcConfig};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub corpus: Arc<LawCorpus>,
    /// De beschikking-procedure uit de wet (RFC-008): de orchestratie
    /// valideert elke statusovergang tegen deze definitie.
    pub procedure: Arc<crate::engine::Procedure>,
    /// De bezwaarprocedure uit de AWB; zelfde mechaniek.
    pub bezwaar_procedure: Arc<crate::engine::Procedure>,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub oidc_config: Option<OidcConfig>,
    pub end_session_url: Option<String>,
    pub base_url: Option<String>,
    pub http_client: reqwest::Client,
}

impl OidcAppState for AppState {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
        self.oidc_client.as_ref()
    }
    fn end_session_url(&self) -> Option<&str> {
        self.end_session_url.as_deref()
    }
    fn oidc_config(&self) -> Option<&OidcConfig> {
        self.oidc_config.as_ref()
    }
    fn is_auth_enabled(&self) -> bool {
        self.oidc_config.is_some()
    }
    fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// Identificatie van een geladen wetversie, vastgelegd in het bewijs bij
/// elk besluit zodat later herleidbaar is met welke teksten is gerekend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WetVersie {
    pub id: String,
    pub valid_from: String,
}

/// The law corpus: YAML sources loaded once at startup. Evaluations run on a
/// per-thread cached engine service keyed on this corpus (see engine.rs).
pub struct LawCorpus {
    pub wpp: String,
    pub regeling: String,
    pub besluit_decentraal: String,
    pub awb: String,
    pub termijnenwet: String,
    /// Subset van de Kieswet (artikel G 1): de registratie-eisen voor een
    /// aanduiding, gebruikt bij de claim-toets in het partijregister.
    pub kieswet: String,
    /// De versies ($id + valid_from) van de geladen teksten, voor het
    /// bewijs bij besluiten.
    pub versies: Vec<WetVersie>,
}

fn wet_versie(yaml: &str) -> Option<WetVersie> {
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).ok()?;
    Some(WetVersie {
        id: doc.get("$id")?.as_str()?.to_string(),
        valid_from: doc.get("valid_from")?.as_str()?.to_string(),
    })
}

impl LawCorpus {
    fn new(
        wpp: String,
        regeling: String,
        besluit_decentraal: String,
        awb: String,
        termijnenwet: String,
        kieswet: String,
    ) -> Self {
        let versies = [
            &wpp,
            &regeling,
            &besluit_decentraal,
            &awb,
            &termijnenwet,
            &kieswet,
        ]
        .into_iter()
        .filter_map(|yaml| wet_versie(yaml))
        .collect();
        Self {
            wpp,
            regeling,
            besluit_decentraal,
            awb,
            termijnenwet,
            kieswet,
            versies,
        }
    }

    pub fn load() -> anyhow::Result<Self> {
        let dir = std::env::var("NAPP_LAW_DIR").unwrap_or_else(|_| "law".to_string());
        let dir = PathBuf::from(dir);
        let read = |rel: &str| -> anyhow::Result<String> {
            let path = dir.join(rel);
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("kan wettekst {} niet lezen: {e}", path.display()))
        };
        Ok(Self::new(
            read("wet_op_de_politieke_partijen/2026-01-01.yaml")?,
            read("regeling_subsidiebedragen/2026-01-01.yaml")?,
            read("besluit_subsidiering_decentrale_politieke_partijen/2026-01-01.yaml")?,
            read("algemene_wet_bestuursrecht/1994-01-01.yaml")?,
            read("algemene_termijnenwet/1964-04-01.yaml")?,
            read("kieswet/1989-09-28.yaml")?,
        ))
    }

    /// De volledige corpus uit de repository, voor tests en fixtures.
    #[cfg(test)]
    pub fn embedded() -> Self {
        Self::new(
            include_str!("../../../corpus-poc/napp/law/wet_op_de_politieke_partijen/2026-01-01.yaml").to_string(),
            include_str!("../../../corpus-poc/napp/law/regeling_subsidiebedragen/2026-01-01.yaml").to_string(),
            include_str!(
                "../../../corpus-poc/napp/law/besluit_subsidiering_decentrale_politieke_partijen/2026-01-01.yaml"
            )
            .to_string(),
            include_str!("../../../corpus-poc/napp/law/algemene_wet_bestuursrecht/1994-01-01.yaml").to_string(),
            include_str!("../../../corpus-poc/napp/law/algemene_termijnenwet/1964-04-01.yaml").to_string(),
            include_str!("../../../corpus-poc/napp/law/kieswet/1989-09-28.yaml").to_string(),
        )
    }
}
