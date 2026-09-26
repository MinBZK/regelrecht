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
    pub stroom: String,
    pub event: String,
    /// Het ontvangstkanaal (`$intake.*`): wie indiende en langs welke weg.
    #[serde(default)]
    pub intake: Value,
    /// De inhoud (`$external.*`).
    #[serde(default)]
    pub external: Map<String, Value>,
    /// Bij `zaak: volgt` de zaak die het gram volgt. Bij `zaak: opent` geeft
    /// de cel het kenmerk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaakkenmerk: Option<String>,
    /// Bij `besluit: volgt` het besluit dat het gram volgt, bij `besluit:
    /// wijzigt` het besluit dat het wijzigt. Bij `besluit: opent` geeft de
    /// cel het kenmerk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besluitkenmerk: Option<String>,
    /// Alleen bij een handeling die het proces uitrekende: de invoer met
    /// herkomst en het receipt, en bij een besluit wat het tot besluit maakt.
    /// Het proces draait de engine, dus het proces stelt dit samen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besluit: Option<Besluitvelden>,
    /// Hoeveel grammen de zaak had toen het proces haar las. De cel legt
    /// alleen vast als dat onder haar slot nog zo is: wat het proces
    /// uitrekende, gold voor de zaak zoals die toen was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaak_grammen: Option<usize>,
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
    pub handelende_actor: Option<crate::gram::HandelendeActor>,
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
pub fn celpad(cel: &str, route: &str) -> String {
    format!("/cellen/{cel}/api/{route}")
}

fn lees<T: DeserializeOwned>(v: Value, wat: &str) -> Result<T, TransportFout> {
    serde_json::from_value(v).map_err(|e| TransportFout::Json(format!("{wat}: {e}")))
}

fn schrijf<T: Serialize>(v: &T) -> Result<Value, TransportFout> {
    serde_json::to_value(v)
        .map_err(|e| TransportFout::Json(format!("het verzoek is geen JSON: {e}")))
}

/// De grammen van een zaak, elk met YAML, zoals de cel ze filtert
/// (`GET zaken/{zaakkenmerk}`). Een proces leest nooit de hele kroniek: het
/// filteren is werk van de cel. Een 404: de cel kent de zaak niet.
pub async fn lees_zaak(
    cel: &dyn Transport,
    id: &str,
    zaakkenmerk: &str,
) -> Result<Vec<MetYaml>, TransportFout> {
    let v = cel
        .haal(&celpad(id, &format!("zaken/{zaakkenmerk}")))
        .await?;
    lees(
        v,
        &format!("de cel gaf geen lijst grammen voor zaak {zaakkenmerk}"),
    )
}

/// De stand van een zaak, zoals de cel haar afleidt (de lexostatus
/// [`ZAAKSTAND`], zie [`Zaakstand`]). Met `eigenaar` (een `$intake`-pad zonder
/// `$intake.` en een waarde) zegt de cel ook of iemand met die waarde de zaak
/// kent. Een 404: de cel kent de zaak niet.
pub async fn zaakstand(
    cel: &dyn Transport,
    id: &str,
    zaakkenmerk: &str,
    eigenaar: Option<(&str, &str)>,
) -> Result<Zaakstand, TransportFout> {
    let mut query = vec![("zaakkenmerk", zaakkenmerk)];
    if let Some((pad, waarde)) = eigenaar {
        query.push((EIGENAAR_PAD, pad));
        query.push((EIGENAAR, waarde));
    }
    let query = serde_urlencoded::to_string(&query)
        .map_err(|e| TransportFout::Json(format!("de vraag is niet te schrijven: {e}")))?;
    let v = cel
        .haal(&celpad(id, &format!("lexostatus/{ZAAKSTAND}?{query}")))
        .await?;
    let l: Lexostatus = lees(v, "de cel gaf geen lexostatus voor de zaak")?;
    Zaakstand::uit(&l).map_err(TransportFout::Json)
}

/// Laat de cel een gram vastleggen (`POST grammen`). Antwoord: het
/// vastgelegde gram met zijn YAML.
pub async fn leg_vast(
    cel: &dyn Transport,
    id: &str,
    verzoek: &Vastlegverzoek,
) -> Result<MetYaml, TransportFout> {
    let v = cel
        .stuur(&celpad(id, "grammen"), &schrijf(verzoek)?)
        .await?;
    lees(v, "het vastgelegde gram is onleesbaar")
}

/// Laat de cel een concept op proef reduceren tot `lexostatus`
/// (`POST lexostatus/<naam>/proef`), met `inputs` (en zo nodig het peil); er
/// wordt niets vastgelegd.
pub async fn proef(
    cel: &dyn Transport,
    id: &str,
    lexostatus: &str,
    concept: &Vastlegverzoek,
    inputs: &Map<String, Value>,
) -> Result<Proefreductie, TransportFout> {
    let body = json!({"concept": schrijf(concept)?, "inputs": inputs});
    let v = cel
        .stuur(
            &celpad(id, &format!("lexostatus/{lexostatus}/proef")),
            &body,
        )
        .await?;
    lees(v, "de proefreductie van de cel is onleesbaar")
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
                "/cellen/c/api/zaken/{z}",
                get(|| async { Json(json!({"niet": "een lijst"})) }),
            )
            .route(
                "/cellen/c/api/grammen",
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
        let fout = lees_zaak(&t, "c", "z").await.unwrap_err();
        assert!(
            matches!(&fout, TransportFout::Json(r) if r.contains("geen lijst grammen")),
            "{fout:?}"
        );
        let fout = leg_vast(&t, "c", &Vastlegverzoek::default())
            .await
            .unwrap_err();
        assert!(
            matches!(&fout, TransportFout::Json(r) if r.contains("onleesbaar")),
            "{fout:?}"
        );
    }
}
