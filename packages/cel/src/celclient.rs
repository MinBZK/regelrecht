//! De cel-client: hoe een proces de cel vraagt waarin het vastlegt. Een
//! proces leest nooit zelf in een kroniek; het vraagt de cel langs haar
//! routes, via een [`Transport`], zoals elke afnemer.
//!
//! Hier staan de verzoeken en antwoorden van die routes als typen, zodat een
//! antwoord dat niet de verwachte vorm heeft een fout is en niet stil een
//! lege waarde wordt.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::gram::{Gram, Invoer, Receipt};
use crate::reductie::{Lexostatus, Zaakstand, EIGENAAR, EIGENAAR_PAD, ZAAKSTAND};
use crate::transport::{Transport, TransportFout};

/// Wat een proces de cel vraagt vast te leggen (`POST /api/grammen`), of op
/// proef te reduceren. De cel bouwt het gram uit haar stroom: het proces
/// geeft alleen de invoer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vastlegverzoek {
    /// Wie laat vastleggen. De cel weigert als dat niet de `recording_actor`
    /// van de stroom is.
    pub actor: String,
    pub stream: String,
    pub event: String,
    /// Het ontvangstkanaal (`$intake.*`): wie indiende en langs welke weg.
    #[serde(default)]
    pub intake: Value,
    /// De inhoud (`$external.*`).
    #[serde(default)]
    pub external: Map<String, Value>,
    /// Per verwijzing van het event het id van het gram waarnaar het nieuwe
    /// gram verwijst (de aanvraag, het besluit). Het id van het nieuwe gram
    /// geeft de cel.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refers_to: BTreeMap<String, String>,
    /// Alleen bij een handeling die het proces uitrekende: de invoer met
    /// herkomst en het receipt, en bij een besluit wat het tot besluit maakt.
    /// Het proces draait de engine, dus het proces stelt dit samen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Besluitvelden>,
    /// Hoeveel grammen de groep van de wortel had toen het proces haar las.
    /// De cel legt alleen vast als dat onder haar slot nog zo is: wat het
    /// proces uitrekende, gold voor de groep zoals die toen was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_grams: Option<usize>,
}

/// De velden van een handeling op een gram, naast de stroomvorm (zie
/// [`crate::handeling::neem`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Besluitvelden {
    #[serde(default)]
    pub legal_character: Option<String>,
    #[serde(default)]
    pub decision_type: Option<String>,
    #[serde(default)]
    pub regulation: Option<String>,
    #[serde(default)]
    pub regulation_valid_from: Option<String>,
    #[serde(default)]
    pub competent_authority: Option<String>,
    #[serde(default)]
    pub acting_actor: Option<crate::gram::HandelendeActor>,
    #[serde(default)]
    pub inputs: BTreeMap<String, Invoer>,
    #[serde(default)]
    pub receipt: Option<Receipt>,
}

/// Een gram zoals de cel het teruggeeft: met zijn YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetYaml {
    pub gram: Gram,
    pub yaml: String,
}

/// Het antwoord op een proefreductie: het gram van het concept en de
/// lexostatus van de kroniek mét dat gram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proefreductie {
    pub gram: Gram,
    pub lexostatus: Lexostatus,
}

/// Een route van een cel.
pub fn celpad(cell: &str, route: &str) -> String {
    format!("/cells/{cell}/api/{route}")
}

fn lees<T: DeserializeOwned>(v: Value, wat: &str) -> Result<T, TransportFout> {
    serde_json::from_value(v).map_err(|e| TransportFout::Json(format!("{wat}: {e}")))
}

fn schrijf<T: Serialize>(v: &T) -> Result<Value, TransportFout> {
    serde_json::to_value(v)
        .map_err(|e| TransportFout::Json(format!("het verzoek is geen JSON: {e}")))
}

/// De grammen met deze wortel, elk met YAML, zoals de cel ze filtert
/// (`GET zaken/{wortel}`). Een proces leest nooit de hele kroniek: het
/// filteren is werk van de cel. Een 404: de cel kent de wortel niet.
pub async fn lees_zaak(
    cell: &dyn Transport,
    id: &str,
    root: &str,
) -> Result<Vec<MetYaml>, TransportFout> {
    let v = cell.haal(&celpad(id, &format!("cases/{root}"))).await?;
    lees(
        v,
        &format!("de cel gaf geen lijst grammen voor wortel {root}"),
    )
}

/// De stand van de groep rond een wortel, zoals de cel haar afleidt (de
/// lexostatus [`ZAAKSTAND`], zie [`Zaakstand`]). Met `eigenaar` (een
/// `$intake`-pad zonder `$intake.` en een waarde) zegt de cel ook of iemand
/// met die waarde de groep kent. Een 404: de cel kent de wortel niet.
pub async fn zaakstand(
    cell: &dyn Transport,
    id: &str,
    root: &str,
    owner: Option<(&str, &str)>,
) -> Result<Zaakstand, TransportFout> {
    let mut query = vec![("root", root)];
    if let Some((path, value)) = owner {
        query.push((EIGENAAR_PAD, path));
        query.push((EIGENAAR, value));
    }
    let query = serde_urlencoded::to_string(&query)
        .map_err(|e| TransportFout::Json(format!("de vraag is niet te schrijven: {e}")))?;
    let v = cell
        .haal(&celpad(id, &format!("lexostatus/{ZAAKSTAND}?{query}")))
        .await?;
    let l: Lexostatus = lees(v, "de cel gaf geen lexostatus voor de wortel")?;
    Zaakstand::uit(&l).map_err(TransportFout::Json)
}

/// Laat de cel een gram vastleggen (`POST grammen`). Antwoord: het
/// vastgelegde gram met zijn YAML.
pub async fn leg_vast(
    cell: &dyn Transport,
    id: &str,
    verzoek: &Vastlegverzoek,
) -> Result<MetYaml, TransportFout> {
    let v = cell.stuur(&celpad(id, "grams"), &schrijf(verzoek)?).await?;
    lees(v, "het vastgelegde gram is onleesbaar")
}

/// Laat de cel een concept op proef reduceren tot `lexostatus`
/// (`POST lexostatus/<naam>/proef`), met `inputs` (en zo nodig het peil); er
/// wordt niets vastgelegd.
pub async fn trial(
    cell: &dyn Transport,
    id: &str,
    lexostatus: &str,
    concept: &Vastlegverzoek,
    inputs: &Map<String, Value>,
) -> Result<Proefreductie, TransportFout> {
    let body = json!({"draft": schrijf(concept)?, "inputs": inputs});
    let v = cell
        .stuur(
            &celpad(id, &format!("lexostatus/{}/trial", url_segment(lexostatus))),
            &body,
        )
        .await?;
    lees(v, "de proefreductie van de cel is onleesbaar")
}

/// Een padsegment zoals het in een url staat: alleen letters, cijfers en
/// `_-.` blijven staan.
pub fn url_segment(t: &str) -> String {
    t.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"_-.".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::Http;
    use axum::routing::get;
    use axum::{Json, Router};
    use std::time::Duration;

    /// Een cel over HTTP die op elke vraag iets antwoordt dat geen gram of
    /// lijst grammen is.
    async fn verkeerde_cel() -> Http {
        let router = Router::new()
            .route(
                "/cells/c/api/cases/{z}",
                get(|| async { Json(json!({"niet": "een lijst"})) }),
            )
            .route(
                "/cells/c/api/grams",
                axum::routing::post(|| async { Json(json!([1, 2])) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let adres = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        Http::new(&format!("http://{adres}"), Duration::from_secs(5)).unwrap()
    }

    #[tokio::test]
    async fn een_onleesbaar_antwoord_over_http_is_een_fout() {
        let t = verkeerde_cel().await;
        let error = lees_zaak(&t, "c", "z").await.unwrap_err();
        assert!(
            matches!(&error, TransportFout::Json(r) if r.contains("geen lijst grammen")),
            "{error:?}"
        );
        let error = leg_vast(&t, "c", &Vastlegverzoek::default())
            .await
            .unwrap_err();
        assert!(
            matches!(&error, TransportFout::Json(r) if r.contains("onleesbaar")),
            "{error:?}"
        );
    }
}
