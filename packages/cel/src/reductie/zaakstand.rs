//! De stand van een zaak: een lexostatus die de runtime aanbiedt voor elke
//! cel met een zaak, onder de naam [`ZAAKSTAND`].
//!
//! Een proces dat in een zaak handelt, moet weten hoe ver die zaak is: welke
//! besluiten erin liggen en welke stages elk besluit doorliep (RFC-008), hoe
//! vaak elk event erin vastligt, wat elk besluit vastlegde (de toestand
//! waarop een latere stage van dat besluit verdergaat), het laatste
//! `op_moment`, en of een aanvrager de zaak kent. Dat zijn afleidingen
//! uit de grammen van de zaak, en afleiden is reductie: dat gebeurt in de cel
//! (paper P:58, P:66), niet in het proces. Het proces leest deze lexostatus,
//! geen grammen.
//!
//! De naam is gebleven; sinds chronolex v0.2.0 is "de zaak" de groep rond
//! een wortel: het gram zonder verwijzing (de aanvraag) met alles wat er via
//! verwijzingen op volgt. Een besluit is een gram met stage BESLUIT; wat
//! ernaar verwijst, volgt het.
//!
//! De runtime biedt haar aan, geen `lexostatussen.yaml`: de groep (de
//! wortel, de verwijzingen en een stage per besluit) is een begrip van de
//! runtime zelf, niet van een casus, en wat een proces erover vraagt is voor
//! elke cel hetzelfde. Een eigen definitie per cel zou in elke cel dezelfde
//! regels herhalen, en het proces zou van hun naam en vorm afhangen. Een cel
//! kan de naam daarom niet zelf gebruiken.
//!
//! De lexostatus gaat nooit naar de engine: ze heeft geen parameters, alleen
//! extra velden ([`Zaakstand`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{Lexostatus, Peil};
use crate::gram::Gram;

/// De naam van de lexostatus. Gereserveerd: geen cel definieert haar zelf.
pub const ZAAKSTAND: &str = "case_state";

/// De optionele inputs voor de vraag of iemand de zaak kent: het
/// `$intake`-pad van het eigenaarveld van een kanaal, en de waarde van wie
/// het vraagt.
pub const EIGENAAR_PAD: &str = "owner_path";
pub const EIGENAAR: &str = "owner";

/// Wat de cel over een zaak afleidt. Staat in de lexostatus als extra
/// velden.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zaakstand {
    /// Hoeveel grammen de groep heeft. Een proces stuurt dit mee als
    /// `wortel_grammen` bij het vastleggen: wat het uitrekende, gold voor de
    /// zaak zoals die toen was.
    pub grams: usize,
    /// Per event (`<stroom>/<event>`) hoeveel grammen er in de zaak liggen.
    pub events: BTreeMap<String, usize>,
    /// Het laatste `op_moment` in de zaak: een feit dat de zaak volgt, ligt
    /// rechtens niet op een eerdere dag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_effective_at: Option<String>,
    /// Per stage (RFC-008) het gram dat haar vastlegde, voor de stages die
    /// bij geen besluit horen (zoals de aanvraag). Elk ligt een keer in de
    /// zaak; de cel dwingt dat af.
    pub stages: BTreeMap<String, Stagestand>,
    /// De besluiten in de zaak, in de volgorde waarin de cel ze vastlegde:
    /// per besluit zijn stages en de grammen die het volgen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Besluitstand>,
    /// Alleen als erom gevraagd is: of er een gram in de zaak ligt waarvan
    /// het veld dat aan het eigenaarpad bindt, de gevraagde waarde heeft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
}

/// Een stage in de zaak: wat het gram van die stage vastlegde. Bij een
/// besluit is dat de toestand waarop een latere stage verdergaat (RFC-008:
/// het besluit is de state container).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stagestand {
    pub stream: String,
    pub event: String,
    pub effective_at: String,
    pub recorded_at: String,
    /// De regeling waarop een besluit rust, als het gram haar noemt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// De velden van het gram: bij een besluit zijn uitkomsten.
    pub fields: Map<String, Value>,
    /// De waarden van de invoer waarmee de engine het uitrekende.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input: BTreeMap<String, Value>,
}

/// Een besluit in de zaak (RFC-008: het besluit is de state container): het
/// gram dat het opende of wijzigde, de stages die het doorliep, en hoeveel
/// grammen van elk event het volgen (zoals betalingen die het uitvoeren).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Besluitstand {
    /// Het id van het gram dat het besluit is.
    pub id: String,
    /// Het event dat het besluit vastlegde.
    pub stream: String,
    pub event: String,
    /// Het besluit dat dit besluit wijzigt, als het een wijziging is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amends: Option<String>,
    /// Per stage het gram dat haar voor dit besluit vastlegde; de stage van
    /// het besluit zelf (zoals BESLUIT) draagt zijn uitkomsten en invoer.
    pub stages: BTreeMap<String, Stagestand>,
    /// Per event (`<stroom>/<event>`) hoeveel grammen dit besluit volgen,
    /// het besluit zelf meegeteld.
    pub events: BTreeMap<String, usize>,
}

impl Besluitstand {
    /// De stage waarin het besluit werd genomen: die van het gram dat het
    /// opende.
    pub fn genomen(&self) -> Option<(&String, &Stagestand)> {
        self.stages
            .iter()
            .find(|(_, s)| s.event == self.event && s.stream == self.stream)
    }

    /// Of het besluit door dit event werd vastgelegd.
    pub fn van(&self, stream: &str, event: &str) -> bool {
        self.stream == stream && self.event == event
    }
}

impl Zaakstand {
    /// Het besluit met dit id.
    pub fn decision(&self, id: &str) -> Option<&Besluitstand> {
        self.decisions.iter().find(|b| b.id == id)
    }

    /// De sleutel van een event in [`Zaakstand::events`].
    pub fn sleutel(stream: &str, event: &str) -> String {
        format!("{stream}/{event}")
    }

    /// Hoeveel grammen van dit event in de zaak liggen.
    pub fn aantal(&self, stream: &str, event: &str) -> usize {
        self.events
            .get(&Self::sleutel(stream, event))
            .copied()
            .unwrap_or(0)
    }

    /// De stand als lexostatus: alles als extra veld.
    pub fn als_lexostatus(&self, root: &str, peil: &Peil) -> Result<Lexostatus, String> {
        let Value::Object(fields) = serde_json::to_value(self).map_err(|e| e.to_string())? else {
            return Err("de zaakstand is geen object".into());
        };
        Ok(Lexostatus {
            root: Some(root.to_string()),
            as_of: peil.as_of.map(|t| t.to_string()),
            known_at: peil.known_at.map(|t| t.to_string()),
            extra_fields: fields.into_iter().collect(),
            ..Lexostatus::leeg(ZAAKSTAND)
        })
    }

    /// De stand uit de lexostatus die de cel gaf.
    pub fn uit(l: &Lexostatus) -> Result<Self, String> {
        let fields: Map<String, Value> = l.extra_fields.clone().into_iter().collect();
        serde_json::from_value(Value::Object(fields))
            .map_err(|e| format!("de cel gaf geen zaakstand: {e}"))
    }
}

/// Leid de stand van een zaak af uit haar grammen, op een peil. `bindt` zegt
/// per gram welke velden aan een `$intake`-pad binden (uit de stroom van de
/// cel); `eigenaar` is het pad en de waarde waarnaar gevraagd wordt. `None`:
/// geen gram van de zaak telt bij dit peil.
pub fn reduceer_zaak<'g>(
    grams: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
    owner: Option<(&str, &str)>,
    bindt: impl Fn(&Gram, &str) -> Vec<String>,
) -> Result<Option<Zaakstand>, String> {
    let mut stand = Zaakstand::default();
    let mut laatste: Option<(chrono::DateTime<chrono::FixedOffset>, String)> = None;
    let mut is_eigenaar = false;
    for g in grams {
        if !peil.laat_door(g)? {
            continue;
        }
        stand.grams += 1;
        *stand
            .events
            .entry(Zaakstand::sleutel(&g.stream.id, &g.name))
            .or_default() += 1;
        let m = g.moment()?;
        if laatste.as_ref().is_none_or(|(l, _)| m > *l) {
            laatste = Some((m, g.effective_at.clone()));
        }
        // Een gram met stage BESLUIT is een besluit (met `wijzigt` een
        // wijziging); een gram dat naar een besluit in de groep verwijst, volgt
        // het. De rest hoort bij de groep zelf (zoals de aanvraag).
        let is_besluit = g.stage.as_deref() == Some(crate::stroom::BESLUIT);
        let gevolgd = stand
            .decisions
            .iter()
            .position(|b| g.refers_to.values().any(|d| *d == b.id));
        let stages = if is_besluit {
            stand.decisions.push(Besluitstand {
                id: g.id.clone(),
                stream: g.stream.id.clone(),
                event: g.name.clone(),
                amends: g.refers_to.get(crate::stroom::WIJZIGT).cloned(),
                ..Besluitstand::default()
            });
            stand.decisions.last_mut().map(|b| {
                *b.events
                    .entry(Zaakstand::sleutel(&g.stream.id, &g.name))
                    .or_default() += 1;
                &mut b.stages
            })
        } else if let Some(i) = gevolgd {
            stand.decisions.get_mut(i).map(|b| {
                *b.events
                    .entry(Zaakstand::sleutel(&g.stream.id, &g.name))
                    .or_default() += 1;
                &mut b.stages
            })
        } else {
            Some(&mut stand.stages)
        };
        if let (Some(stages), Some(s)) = (stages, &g.stage) {
            // Een stage ligt een keer in een besluit (of in de zaak); lag er
            // toch een tweede (een oudere kroniek), dan telt de laatste in de
            // tijd.
            let later = match stages.get(s) {
                None => true,
                Some(eerder) => {
                    let e = crate::datum::moment(&eerder.effective_at)?;
                    m >= e
                }
            };
            if later {
                stages.insert(
                    s.clone(),
                    Stagestand {
                        stream: g.stream.id.clone(),
                        event: g.name.clone(),
                        effective_at: g.effective_at.clone(),
                        recorded_at: g.recorded_at.clone(),
                        regulation: g.regulation.clone(),
                        fields: g.fields.clone(),
                        input: g
                            .inputs
                            .iter()
                            .map(|(k, i)| (k.clone(), i.value.clone()))
                            .collect(),
                    },
                );
            }
        }
        if let Some((path, value)) = owner {
            is_eigenaar |= bindt(g, path)
                .iter()
                .any(|field| g.field(field).and_then(Value::as_str) == Some(value));
        }
    }
    if stand.grams == 0 {
        return Ok(None);
    }
    stand.latest_effective_at = laatste.map(|(_, t)| t);
    stand.owner = owner.map(|_| is_eigenaar);
    Ok(Some(stand))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{Invoer, StroomVerwijzing};
    use crate::synthese::Herkomst;
    use serde_json::json;

    fn gram(name: &str, stage: Option<&str>, effective_at: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            id: uuid::Uuid::now_v7().to_string(),
            type_: "act".into(),
            subtype: None,
            stage: stage.map(str::to_string),
            name: name.into(),
            chronicle: "voorbeeld".into(),
            recording_actor: "voorbeeld_actor".into(),
            legal_basis: vec!["voorbeeldregeling#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            acting_actor: None,
            effective_at: effective_at.into(),
            effective_at_legal_basis: None,
            recorded_at: "2025-03-12T10:00:00+01:00".into(),
            refers_to: BTreeMap::new(),
            stream: StroomVerwijzing {
                id: "voorbeeldstroom".into(),
                sha256: "0".repeat(64),
            },
            provenance: None,
            fields: fields.as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
            tijden: Default::default(),
            root: Some("z1".into()),
        }
    }

    fn case() -> Vec<Gram> {
        let mut decision = gram(
            "besluit_genomen",
            Some("BESLUIT"),
            "2025-03-10T00:00:00+01:00",
            json!({"bedrag": 100}),
        );
        decision.regulation = Some("voorbeeldregeling".into());
        decision.inputs.insert(
            "x".into(),
            Invoer {
                value: json!(4),
                provenance: Herkomst::Handler,
            },
        );
        let betaald = |bedrag: i64| {
            let mut g = gram(
                "betaald",
                None,
                "2025-03-11T00:00:00+01:00",
                json!({"bedrag": bedrag}),
            );
            g.refers_to.insert("decision".into(), decision.id.clone());
            g
        };
        let (b40, b60) = (betaald(40), betaald(60));
        vec![
            gram(
                "aanvraag_ontvangen",
                Some("AANVRAAG"),
                "2025-03-01T09:00:00+01:00",
                json!({"nummer": "12345678"}),
            ),
            decision,
            b40,
            b60,
        ]
    }

    fn bindt(g: &Gram, path: &str) -> Vec<String> {
        if g.name == "aanvraag_ontvangen" && path == "kanaal.nummer" {
            vec!["nummer".into()]
        } else {
            Vec::new()
        }
    }

    /// De cel leidt af wat een proces over de zaak vraagt: de stages met
    /// wat hun gram vastlegde, het aantal per event, het laatste moment.
    #[test]
    fn de_stand_van_een_zaak() {
        let z = case();
        let s = reduceer_zaak(&z, &Peil::default(), None, bindt)
            .unwrap()
            .unwrap();
        assert_eq!(s.grams, 4);
        assert_eq!(s.aantal("voorbeeldstroom", "betaald"), 2);
        assert_eq!(s.aantal("voorbeeldstroom", "bestaat_niet"), 0);
        assert_eq!(s.stages.keys().collect::<Vec<_>>(), ["AANVRAAG"]);
        // Het besluit is een eigen besluit in de groep; de betalingen die
        // ernaar verwijzen, volgen het.
        assert_eq!(s.decisions.len(), 1);
        assert_eq!(s.decisions[0].id, z[1].id);
        assert_eq!(s.decisions[0].events["voorbeeldstroom/betaald"], 2);
        let b = &s.decisions[0].stages["BESLUIT"];
        assert_eq!(b.event, "besluit_genomen");
        assert_eq!(b.fields["bedrag"], json!(100));
        assert_eq!(b.input["x"], json!(4));
        assert_eq!(b.regulation.as_deref(), Some("voorbeeldregeling"));
        assert_eq!(
            s.latest_effective_at.as_deref(),
            Some("2025-03-11T00:00:00+01:00")
        );
        assert_eq!(s.owner, None);
        // Heen en terug door de lexostatus.
        let l = s.als_lexostatus("z1", &Peil::default()).unwrap();
        assert!(l.parameters.is_empty(), "gaat nooit naar de engine");
        assert_eq!(Zaakstand::uit(&l).unwrap(), s);
    }

    /// Of iemand de zaak kent, zegt de cel: een gram waarvan het veld dat
    /// aan het eigenaarpad bindt, de waarde heeft.
    #[test]
    fn de_eigenaar_van_een_zaak() {
        let z = case();
        let ja = reduceer_zaak(
            &z,
            &Peil::default(),
            Some(("kanaal.nummer", "12345678")),
            bindt,
        )
        .unwrap()
        .unwrap();
        assert_eq!(ja.owner, Some(true));
        let nee = reduceer_zaak(
            &z,
            &Peil::default(),
            Some(("kanaal.nummer", "87654321")),
            bindt,
        )
        .unwrap()
        .unwrap();
        assert_eq!(nee.owner, Some(false));
    }

    /// Op een peil telt alleen wat toen gold; een zaak zonder gram bij het
    /// peil heeft geen stand.
    #[test]
    fn de_stand_op_een_peil() {
        let z = case();
        let peil = Peil::op(crate::datum::Tijdpunt::lees("p", "2025-03-05").unwrap());
        let s = reduceer_zaak(&z, &peil, None, bindt).unwrap().unwrap();
        assert_eq!(s.grams, 1);
        assert!(!s.stages.contains_key("BESLUIT"));
        let vroeg = Peil::op(crate::datum::Tijdpunt::lees("p", "2025-02-01").unwrap());
        assert!(reduceer_zaak(&z, &vroeg, None, bindt).unwrap().is_none());
    }
}
