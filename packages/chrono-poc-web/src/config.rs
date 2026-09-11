//! De configuratie, uit de omgeving, één keer bij het starten gelezen.

use std::time::Duration;

pub use regelrecht_auth::OidcConfig;

use crate::sources::Source;

/// Hoe lang een sessie zonder verkeer haar wereld houdt.
///
/// Eén uur, en dezelfde waarde voor de sessiecookie en voor de wereld erachter:
/// twee verschillende getallen leveren één van twee verwarringen op — een cookie
/// die een wereld aanwijst die er niet meer is, of een wereld die geld kost
/// zonder dat er nog een browser bij hoort.
pub const SESSION_TTL: Duration = Duration::from_secs(60 * 60);

/// Hoe vaak er gekeken wordt of er werelden verlopen zijn.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// De rol die een gebruiker minimaal moet hebben voor `/api/*`.
///
/// Standaard de leesrol van de editor, zodat deze PoC achter de bestaande login
/// staat zonder dat er in Keycloak eerst een rol bij moet. Een eigen rol
/// (`simulator-reader`) is één env-variabele, niet een codewijziging.
pub const DEFAULT_REQUIRED_ROLE: &str = "editor-reader";

/// Alles wat dit proces uit zijn omgeving haalt.
#[derive(Clone)]
pub struct AppConfig {
    /// OIDC, of `None` voor de auth-uit-modus (lokaal).
    pub oidc: Option<OidcConfig>,
    /// `BASE_URL`, voor de redirect-URL's van de login.
    pub base_url: Option<String>,
    /// De rol waarachter `/api/*` staat.
    ///
    /// `&'static str` omdat de route-gate van `regelrecht-auth` dat vraagt: de
    /// middleware wordt één keer gebouwd en leeft even lang als het proces. De
    /// waarde uit de omgeving wordt daarom bij het starten geleaked — één keer,
    /// voor één string.
    pub required_role: &'static str,
    /// Waar het wereldbestand staat.
    pub world: Source,
    /// Waar de regelingen staan; `None` laat de simulator kiezen
    /// (`REGULATION_PATH`, anders het corpus in deze checkout).
    pub corpus: Option<Source>,
    /// De sleutel waaronder het GitHub-token opgezocht wordt; `None` gebruikt de
    /// reponaam van de bron.
    pub auth_ref: Option<String>,
    /// De map met de gebouwde frontend.
    pub static_dir: String,
    /// De poort waarop dit proces luistert.
    pub port: u16,
}

impl AppConfig {
    /// Lees de configuratie, of stop het proces met de reden.
    pub fn from_env() -> Self {
        match Self::try_from_env() {
            Ok(config) => config,
            Err(e) => {
                tracing::error!("{e}");
                std::process::exit(1);
            }
        }
    }

    /// Lees de configuratie en geef de eerste fout terug.
    pub fn try_from_env() -> Result<Self, String> {
        let oidc = regelrecht_auth::parse_oidc_from_env()?;
        if oidc.is_some() {
            tracing::info!("OIDC-authenticatie staat aan");
        } else {
            tracing::warn!(
                "OIDC-authenticatie staat UIT — elke route is open. Dit is de lokale modus; \
                 draai deze configuratie niet in productie."
            );
        }
        let base_url = regelrecht_auth::parse_base_url()?;

        let world = match env_value("CHRONO_POC_WORLD_SOURCE") {
            Some(spec) => {
                Source::parse(&spec).map_err(|e| format!("CHRONO_POC_WORLD_SOURCE: {e}"))?
            }
            None => {
                return Err("CHRONO_POC_WORLD_SOURCE is niet gezet. Verwacht \
                     'local:<pad-naar-wereldbestand>' of \
                     'github:<owner>/<repo>@<ref>:<pad-naar-wereldbestand>'."
                    .to_string())
            }
        };
        let corpus = match env_value("CHRONO_POC_CORPUS_SOURCE") {
            Some(spec) => {
                Some(Source::parse(&spec).map_err(|e| format!("CHRONO_POC_CORPUS_SOURCE: {e}"))?)
            }
            None => None,
        };

        // `leak` en geen `Box::leak` met een cache: dit gebeurt één keer per
        // proces, voor één string, omdat de route-gate een `&'static str` wil.
        let required_role: &'static str = match env_value("CHRONO_POC_REQUIRED_ROLE") {
            Some(role) => role.leak(),
            None => DEFAULT_REQUIRED_ROLE,
        };
        tracing::info!(role = %required_role, "/api/* staat achter deze rol");

        let port = match env_value("CHRONO_POC_PORT") {
            Some(raw) => raw
                .parse()
                .map_err(|e| format!("CHRONO_POC_PORT={raw:?} is geen poortnummer: {e}"))?,
            None => 8000,
        };

        Ok(Self {
            oidc,
            base_url,
            required_role,
            world,
            corpus,
            auth_ref: env_value("CHRONO_POC_AUTH_REF"),
            static_dir: env_value("STATIC_DIR").unwrap_or_else(|| "static".to_string()),
            port,
        })
    }

    /// Staat de login aan?
    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}

/// Een env-variabele, waarbij "leeg" hetzelfde is als "niet gezet".
///
/// Een deployment die een variabele op de lege string zet, bedoelt "geen
/// waarde"; zonder dit zou dat een lege bron of een lege rol opleveren.
fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
