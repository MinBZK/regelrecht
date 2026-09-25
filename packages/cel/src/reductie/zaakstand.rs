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
//! De runtime biedt haar aan, geen `lexostatussen.yaml`: de zaak (`zaak:
//! opent | volgt`, het zaakkenmerk, het besluitkenmerk en een stage per
//! besluit) is een begrip van de
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
pub const ZAAKSTAND: &str = "zaakstand";

/// De optionele inputs voor de vraag of iemand de zaak kent: het
/// `$intake`-pad van het eigenaarveld van een kanaal, en de waarde van wie
/// het vraagt.
pub const EIGENAAR_PAD: &str = "eigenaar_pad";
pub const EIGENAAR: &str = "eigenaar";

/// Wat de cel over een zaak afleidt. Staat in de lexostatus als extra
/// velden.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zaakstand {
    /// Hoeveel grammen de zaak heeft. Een proces stuurt dit mee als
    /// `zaak_grammen` bij het vastleggen: wat het uitrekende, gold voor de
    /// zaak zoals die toen was.
    pub grammen: usize,
    /// Per event (`<stroom>/<event>`) hoeveel grammen er in de zaak liggen.
    pub events: BTreeMap<String, usize>,
    /// Het laatste `op_moment` in de zaak: een feit dat de zaak volgt, ligt
    /// rechtens niet op een eerdere dag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laatste_op_moment: Option<String>,
    /// Per stage (RFC-008) het gram dat haar vastlegde, voor de stages die
    /// bij geen besluit horen (zoals de aanvraag). Elk ligt een keer in de
    /// zaak; de cel dwingt dat af.
    pub stages: BTreeMap<String, Stagestand>,
    /// De besluiten in de zaak, in de volgorde waarin de cel ze vastlegde:
    /// per besluit zijn stages en de grammen die het volgen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub besluiten: Vec<Besluitstand>,
    /// Alleen als erom gevraagd is: of er een gram in de zaak ligt waarvan
    /// het veld dat aan het eigenaarpad bindt, de gevraagde waarde heeft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenaar: Option<bool>,
}

/// Een stage in de zaak: wat het gram van die stage vastlegde. Bij een
/// besluit is dat de toestand waarop een latere stage verdergaat (RFC-008:
/// het besluit is de state container).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stagestand {
    pub stroom: String,
    pub event: String,
    pub op_moment: String,
    pub vastgelegd_op: String,
    /// De regeling waarop een besluit rust, als het gram haar noemt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// De velden van het gram: bij een besluit zijn uitkomsten.
    pub velden: Map<String, Value>,
    /// De waarden van de invoer waarmee de engine het uitrekende.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub invoer: BTreeMap<String, Value>,
}

/// Een besluit in de zaak (RFC-008: het besluit is de state container): het
/// gram dat het opende of wijzigde, de stages die het doorliep, en hoeveel
/// grammen van elk event het volgen (zoals betalingen die het uitvoeren).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Besluitstand {
    pub besluitkenmerk: String,
    /// Het event dat het besluit vastlegde.
    pub stroom: String,
    pub event: String,
    /// Het besluit dat dit besluit wijzigt, als het een wijziging is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wijzigt: Option<String>,
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
            .find(|(_, s)| s.event == self.event && s.stroom == self.stroom)
    }

    /// Of het besluit door dit event werd vastgelegd.
    pub fn van(&self, stroom: &str, event: &str) -> bool {
        self.stroom == stroom && self.event == event
    }
}

impl Zaakstand {
    /// Het besluit met dit kenmerk.
    pub fn besluit(&self, kenmerk: &str) -> Option<&Besluitstand> {
        self.besluiten.iter().find(|b| b.besluitkenmerk == kenmerk)
    }

    /// De sleutel van een event in [`Zaakstand::events`].
    pub fn sleutel(stroom: &str, event: &str) -> String {
        format!("{stroom}/{event}")
    }

    /// Hoeveel grammen van dit event in de zaak liggen.
    pub fn aantal(&self, stroom: &str, event: &str) -> usize {
        self.events
            .get(&Self::sleutel(stroom, event))
            .copied()
            .unwrap_or(0)
    }

    /// De stand als lexostatus: alles als extra veld.
    pub fn als_lexostatus(&self, zaakkenmerk: &str, peil: &Peil) -> Result<Lexostatus, String> {
        let Value::Object(velden) = serde_json::to_value(self).map_err(|e| e.to_string())? else {
            return Err("de zaakstand is geen object".into());
        };
        Ok(Lexostatus {
            zaakkenmerk: Some(zaakkenmerk.to_string()),
            peilmoment: peil.peilmoment.map(|t| t.to_string()),
            bekend_op: peil.bekend_op.map(|t| t.to_string()),
            extra_velden: velden.into_iter().collect(),
            ..Lexostatus::leeg(ZAAKSTAND)
        })
    }

    /// De stand uit de lexostatus die de cel gaf.
    pub fn uit(l: &Lexostatus) -> Result<Self, String> {
        let velden: Map<String, Value> = l.extra_velden.clone().into_iter().collect();
        serde_json::from_value(Value::Object(velden))
            .map_err(|e| format!("de cel gaf geen zaakstand: {e}"))
    }
}

/// Leid de stand van een zaak af uit haar grammen, op een peil. `bindt` zegt
/// per gram welke velden aan een `$intake`-pad binden (uit de stroom van de
/// cel); `eigenaar` is het pad en de waarde waarnaar gevraagd wordt. `None`:
/// geen gram van de zaak telt bij dit peil.
pub fn reduceer_zaak<'g>(
    grammen: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
    eigenaar: Option<(&str, &str)>,
    bindt: impl Fn(&Gram, &str) -> Vec<String>,
) -> Result<Option<Zaakstand>, String> {
    let mut stand = Zaakstand::default();
    let mut laatste: Option<(chrono::DateTime<chrono::FixedOffset>, String)> = None;
    let mut is_eigenaar = false;
    for g in grammen {
        if !peil.laat_door(g)? {
            continue;
        }
        stand.grammen += 1;
        *stand
            .events
            .entry(Zaakstand::sleutel(&g.stroom.id, &g.name))
            .or_default() += 1;
        let m = g.moment()?;
        if laatste.as_ref().is_none_or(|(l, _)| m > *l) {
            laatste = Some((m, g.op_moment.clone()));
        }
        let stages = match (&g.besluitkenmerk, g.besluit) {
            (Some(k), Some(rol)) => {
                let i = stand.besluiten.iter().position(|b| &b.besluitkenmerk == k);
                let b = match i {
                    Some(i) => stand.besluiten.get_mut(i),
                    None if rol.is_besluit() => {
                        stand.besluiten.push(Besluitstand {
                            besluitkenmerk: k.clone(),
                            stroom: g.stroom.id.clone(),
                            event: g.name.clone(),
                            wijzigt: g.wijzigt.clone(),
                            ..Besluitstand::default()
                        });
                        stand.besluiten.last_mut()
                    }
                    // Een gram dat een besluit volgt dat niet in de zaak
                    // ligt: de cel legt dat niet vast.
                    None => None,
                };
                b.map(|b| {
                    *b.events
                        .entry(Zaakstand::sleutel(&g.stroom.id, &g.name))
                        .or_default() += 1;
                    &mut b.stages
                })
            }
            _ => Some(&mut stand.stages),
        };
        if let (Some(stages), Some(s)) = (stages, &g.stage) {
            // Een stage ligt een keer in een besluit (of in de zaak); lag er
            // toch een tweede (een oudere kroniek), dan telt de laatste in de
            // tijd.
            let later = match stages.get(s) {
                None => true,
                Some(eerder) => {
                    let e = crate::datum::moment(&eerder.op_moment)?;
                    m >= e
                }
            };
            if later {
                stages.insert(
                    s.clone(),
                    Stagestand {
                        stroom: g.stroom.id.clone(),
                        event: g.name.clone(),
                        op_moment: g.op_moment.clone(),
                        vastgelegd_op: g.vastgelegd_op.clone(),
                        regulation: g.regulation.clone(),
                        velden: g.fields.clone(),
                        invoer: g
                            .inputs
                            .iter()
                            .map(|(k, i)| (k.clone(), i.waarde.clone()))
                            .collect(),
                    },
                );
            }
        }
        if let Some((pad, waarde)) = eigenaar {
            is_eigenaar |= bindt(g, pad)
                .iter()
                .any(|veld| g.veld(veld).and_then(Value::as_str) == Some(waarde));
        }
    }
    if stand.grammen == 0 {
        return Ok(None);
    }
    stand.laatste_op_moment = laatste.map(|(_, t)| t);
    stand.eigenaar = eigenaar.map(|_| is_eigenaar);
    Ok(Some(stand))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{Invoer, StroomVerwijzing};
    use crate::stroom::Zaak;
    use crate::synthese::Herkomst;
    use serde_json::json;

    fn gram(name: &str, stage: Option<&str>, op_moment: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            type_: "handeling".into(),
            soort: None,
            stage: stage.map(str::to_string),
            name: name.into(),
            chronicle: "voorbeeld".into(),
            recording_actor: "voorbeeld_actor".into(),
            grondslag: vec!["voorbeeldregeling#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            handelende_actor: None,
            op_moment: op_moment.into(),
            op_moment_grondslag: None,
            vastgelegd_op: "2025-03-12T10:00:00+01:00".into(),
            zaak: Zaak::Volgt,
            zaakkenmerk: Some("z1".into()),
            besluit: None,
            besluitkenmerk: None,
            wijzigt: None,
            stroom: StroomVerwijzing {
                id: "voorbeeldstroom".into(),
                sha256: "0".repeat(64),
            },
            herkomst: None,
            fields: fields.as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
        }
    }

    fn zaak() -> Vec<Gram> {
        let mut besluit = gram(
            "besluit_genomen",
            Some("BESLUIT"),
            "2025-03-10T00:00:00+01:00",
            json!({"bedrag": 100}),
        );
        besluit.regulation = Some("voorbeeldregeling".into());
        besluit.inputs.insert(
            "x".into(),
            Invoer {
                waarde: json!(4),
                herkomst: Herkomst::Behandelaar,
            },
        );
        vec![
            gram(
                "aanvraag_ontvangen",
                Some("AANVRAAG"),
                "2025-03-01T09:00:00+01:00",
                json!({"nummer": "12345678"}),
            ),
            besluit,
            gram(
                "betaald",
                None,
                "2025-03-11T00:00:00+01:00",
                json!({"bedrag": 40}),
            ),
            gram(
                "betaald",
                None,
                "2025-03-11T00:00:00+01:00",
                json!({"bedrag": 60}),
            ),
        ]
    }

    fn bindt(g: &Gram, pad: &str) -> Vec<String> {
        if g.name == "aanvraag_ontvangen" && pad == "kanaal.nummer" {
            vec!["nummer".into()]
        } else {
            Vec::new()
        }
    }

    /// De cel leidt af wat een proces over de zaak vraagt: de stages met
    /// wat hun gram vastlegde, het aantal per event, het laatste moment.
    #[test]
    fn de_stand_van_een_zaak() {
        let z = zaak();
        let s = reduceer_zaak(&z, &Peil::default(), None, bindt)
            .unwrap()
            .unwrap();
        assert_eq!(s.grammen, 4);
        assert_eq!(s.aantal("voorbeeldstroom", "betaald"), 2);
        assert_eq!(s.aantal("voorbeeldstroom", "bestaat_niet"), 0);
        assert_eq!(s.stages.keys().collect::<Vec<_>>(), ["AANVRAAG", "BESLUIT"]);
        let b = &s.stages["BESLUIT"];
        assert_eq!(b.event, "besluit_genomen");
        assert_eq!(b.velden["bedrag"], json!(100));
        assert_eq!(b.invoer["x"], json!(4));
        assert_eq!(b.regulation.as_deref(), Some("voorbeeldregeling"));
        assert_eq!(
            s.laatste_op_moment.as_deref(),
            Some("2025-03-11T00:00:00+01:00")
        );
        assert_eq!(s.eigenaar, None);
        // Heen en terug door de lexostatus.
        let l = s.als_lexostatus("z1", &Peil::default()).unwrap();
        assert!(l.parameters.is_empty(), "gaat nooit naar de engine");
        assert_eq!(Zaakstand::uit(&l).unwrap(), s);
    }

    /// Of iemand de zaak kent, zegt de cel: een gram waarvan het veld dat
    /// aan het eigenaarpad bindt, de waarde heeft.
    #[test]
    fn de_eigenaar_van_een_zaak() {
        let z = zaak();
        let ja = reduceer_zaak(
            &z,
            &Peil::default(),
            Some(("kanaal.nummer", "12345678")),
            bindt,
        )
        .unwrap()
        .unwrap();
        assert_eq!(ja.eigenaar, Some(true));
        let nee = reduceer_zaak(
            &z,
            &Peil::default(),
            Some(("kanaal.nummer", "87654321")),
            bindt,
        )
        .unwrap()
        .unwrap();
        assert_eq!(nee.eigenaar, Some(false));
    }

    /// Op een peil telt alleen wat toen gold; een zaak zonder gram bij het
    /// peil heeft geen stand.
    #[test]
    fn de_stand_op_een_peil() {
        let z = zaak();
        let peil = Peil::op(crate::datum::Tijdpunt::lees("p", "2025-03-05").unwrap());
        let s = reduceer_zaak(&z, &peil, None, bindt).unwrap().unwrap();
        assert_eq!(s.grammen, 1);
        assert!(!s.stages.contains_key("BESLUIT"));
        let vroeg = Peil::op(crate::datum::Tijdpunt::lees("p", "2025-02-01").unwrap());
        assert!(reduceer_zaak(&z, &vroeg, None, bindt).unwrap().is_none());
    }
}
