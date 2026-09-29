//! Een proces: een map onder `PROCESSES_PATH`, geladen en gecontroleerd.
//!
//! Een proces is van de actor. Het informeert (synthese van lexostatussen uit
//! cellen), concludeert (het besluit) en laat een cel vastleggen. Het legt zelf
//! niets vast en reduceert zelf niets: "reductie vindt altijd plaats ín de
//! cel waar de betreffende chronolexogrammen zijn vastgelegd, op verzoek van
//! een businessproces" (positionpaper). De cel waarin het proces vastlegt is
//! voor het proces een bron zoals elke andere; alleen haar definities (welke
//! stromen en lexostatussen ze heeft) leest het rechtstreeks, voor de
//! controles bij het opstarten en voor de velden van het formulier.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::cel::Cell;
use crate::config::{Portal, ProcesDefinitie, RijenDefinitie, SyntheseBron, VoorbeeldenDefinitie};
use crate::controle;
use crate::formulier::{self, Formulier};
use crate::gezag;
use crate::kanaal;
use crate::origin;
use crate::rijen;
use crate::stroom::{Binding, Event, Stroom};
use crate::voorbeelden::{self, Voorbeelden};

/// Een geladen proces dat de controles bij het opstarten doorstond.
pub struct Proces {
    pub definitie: ProcesDefinitie,
    /// De map van het proces; paden in `proces.yaml` zijn hier relatief aan.
    pub map: PathBuf,
    /// De cel waarin het proces vastlegt: van het portaal, de werkvoorraad,
    /// het besluit en de bronnen van de zaak. Ze draait in deze runtime.
    pub cell: Arc<Cell>,
    /// Het corpus, gedeeld door de hele runtime.
    pub service: Arc<LawExecutionService>,
    pub form: Option<Formulier>,
    /// Standaardgegevens per handeling (leeg zonder `voorbeelden`).
    pub examples: Voorbeelden,
    /// Wat de controle op de herkomst van de parameters zag, maar geen reden
    /// is om niet te starten (zie [`crate::origin`]).
    pub warnings: Vec<String>,
    /// Het tijdvak dat het portaal laat kiezen, als het aanbod er een vraagt.
    pub window: Option<Tijdvak>,
    /// Het bevoegd gezag waarvoor het proces handelt, uit `namens` (zie
    /// [`crate::gezag`]).
    pub authority: Option<String>,
}

/// Het tijdvak van het aanbod: de parameter met origin BELANGHEBBENDE en
/// `rol: TIJDVAK` (zie [`crate::origin`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Tijdvak {
    pub parameter: String,
    /// Het veld van het concept (`external`) waaruit de toets-lexostatus de
    /// parameter afleidt, als ze dat doet. Het portaal vult het vooraf in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Proces {
    /// Laad een proces uit zijn map en controleer het tegen de cellen van de
    /// runtime. Elke fout komt terug, en elke fout noemt het proces. De
    /// controles op synthese en handelingen staan in [`crate::synthese::controleer`]
    /// en [`crate::handeling::controleer`]; de runtime roept ze aan.
    pub fn laad(
        map: &Path,
        cells: &BTreeMap<String, Arc<Cell>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let name = map
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let definitie = ProcesDefinitie::laad(map).map_err(|f| met_proces(&name, f))?;
        let id = definitie.id.clone();
        Self::laad_definitie(definitie, map, cells, service).map_err(|f| met_proces(&id, f))
    }

    fn laad_definitie(
        mut definitie: ProcesDefinitie,
        map: &Path,
        cells: &BTreeMap<String, Arc<Cell>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let cell = de_cel(&definitie, cells)?;
        voeg_wetbronnen_toe(&mut definitie, &cell);
        let mut fouten = Vec::new();
        fouten.extend(actor_legt_vast(&definitie, &cell));
        let authority = gezag::los_op(&definitie, &service)
            .map_err(|f| fouten.extend(f))
            .ok()
            .flatten();
        let portaal_event = definitie
            .portal
            .as_ref()
            .and_then(|p| cell.event(&p.stream, &p.event))
            .map(|(_, e)| e);
        fouten.extend(kanaal::controleer_proces(
            &definitie,
            portaal_event,
            &service,
        ));
        fouten.extend(crate::synthese::grondslagen(&definitie, &service));
        if let Some(p) = &definitie.portal {
            fouten.extend(controle::portal(
                &cell.streams,
                &cell.lexostatuses,
                p,
                &service,
                &kanaal::portaal_intake_paden(&definitie),
            ));
            for r in &p.assessment.rows {
                fouten.extend(rijen::controleer(
                    "assessment",
                    r,
                    &[p.assessment.lexostatus.as_str()],
                    "niet de toets-lexostatus",
                    &definitie,
                    &cell,
                ));
            }
        }
        fouten.extend(crate::handeling::bereid_voor(
            &mut definitie,
            authority.as_deref(),
            &service,
            &cell,
        ));
        let form = match definitie.portal.as_ref().and_then(|p| p.form.as_ref()) {
            Some(f) => formulier::laad(&map.join(&f.path), &f.screen)
                .map_err(|e| fouten.push(e))
                .ok(),
            None => None,
        };
        // Elke grondslag in het formulier wijst een geladen artikel aan, en
        // een lid dat het artikel heeft.
        for (waar, g) in form.iter().flat_map(Formulier::grondslagen) {
            if let Err(f) = crate::regelingen::geldig(&service, &g) {
                fouten.push(format!("formulier, {waar}: {f}"));
            }
        }
        let examples = match &definitie.examples {
            Some(v) => {
                fouten.extend(voorbeelden_zonder_handeling(&definitie, v));
                voorbeelden::laad(map, v, &definitie)
                    .map_err(|f| fouten.extend(f))
                    .unwrap_or_default()
            }
            None => Voorbeelden::default(),
        };
        if !fouten.is_empty() {
            return Err(fouten);
        }
        Ok(Self {
            definitie,
            map: map.to_path_buf(),
            cell,
            service,
            form,
            examples,
            warnings: Vec::new(),
            window: None,
            authority,
        })
    }

    pub fn id(&self) -> &str {
        &self.definitie.id
    }

    /// De controle op de herkomst van de parameters (zie [`crate::origin`]).
    /// Het formulier van elke handeling volgt eruit, en de waarschuwingen
    /// bewaart het proces; de fouten komen terug.
    pub fn controleer_herkomst(&mut self, cells: &BTreeMap<String, Arc<Cell>>) -> Vec<String> {
        let c = origin::controleer(&self.definitie, &self.cell, cells, &self.service);
        if let Some(b) = self.definitie.handling.as_mut() {
            for h in &mut b.actions {
                h.verdicts = origin::verdicts(&c, &self.service, &h.name);
            }
        }
        crate::handeling::zet_formulier(&mut self.definitie, &self.service, &self.cell);
        self.warnings = c.warnings;
        self.window = c.window.map(|parameter| Tijdvak {
            field: self.concept_veld(&parameter),
            parameter,
        });
        c.fouten
    }

    /// Het veld van het concept dat de toets-lexostatus leest om een
    /// parameter af te leiden: een `$external`-sleutel van het portaal-event.
    fn concept_veld(&self, parameter: &str) -> Option<String> {
        let p = self.portal()?;
        let (_, event) = self.portaal_event()?;
        let derivation = self
            .cell
            .lexostatuses
            .lexostatus(&p.assessment.lexostatus)?
            .reduction
            .derivations
            .get(parameter)?;
        let [path] = derivation.gelezen_paden()[..] else {
            return None;
        };
        event.bladeren().into_iter().find_map(|b| match b.binding {
            Binding::External(sleutel) if b.path == path && !sleutel.contains('.') => Some(sleutel),
            _ => None,
        })
    }

    /// Het portaalblok, als het proces een portaal heeft.
    pub fn portal(&self) -> Option<&Portal> {
        self.definitie.portal.as_ref()
    }

    /// De stroom en het event waarin het portaal laat vastleggen.
    pub fn portaal_event(&self) -> Option<(&Stroom, &Event)> {
        let p = self.portal()?;
        self.cell.event(&p.stream, &p.event)
    }

    /// De rijen-definities van de toets (synthese per regel).
    pub fn toets_rijen(&self) -> &[RijenDefinitie] {
        self.portal()
            .map(|p| p.assessment.rows.as_slice())
            .unwrap_or_default()
    }

    /// De handelingen van de behandeling, in de volgorde van `proces.yaml`.
    pub fn actions(&self) -> &[crate::config::HandelingDefinitie] {
        self.definitie
            .handling
            .as_ref()
            .map(|b| b.actions.as_slice())
            .unwrap_or_default()
    }
}

/// De lexostatussen die de wet leest in de cel waarin het proces vastlegt,
/// als bronnen van de zaak (`zaak: true`), na de bronnen van de zaak die
/// `proces.yaml` zelf noemt. Welk artikel welk feit leest, staat in de wet
/// (`produces.extensions.chronolex.leest`); het proces hoeft ze niet op te
/// sommen. Een bron die er al staat, blijft staan.
fn voeg_wetbronnen_toe(definitie: &mut ProcesDefinitie, cell: &Cell) {
    let after = definitie
        .synthesis
        .iter()
        .position(|b| !b.case)
        .unwrap_or(definitie.synthesis.len());
    let nieuw: Vec<SyntheseBron> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| d.law.is_some())
        .filter(|d| {
            !definitie
                .synthesis
                .iter()
                .any(|b| b.case && b.lexostatus == d.name)
        })
        .map(|d| SyntheseBron {
            cell: cell.id().to_string(),
            url: None,
            regulation: None,
            lexostatus: d.name.clone(),
            case: true,
            input: Default::default(),
            parameters: Default::default(),
            extra_fields: Vec::new(),
            legal_basis: Vec::new(),
        })
        .collect();
    definitie.synthesis.splice(after..after, nieuw);
}

/// De cel waarin het proces vastlegt. Het portaal, de werkvoorraad, het
/// besluit en de bronnen van de zaak noemen dezelfde cel: in deze stap
/// handelt een proces over de zaken van een cel, en die cel draait in deze
/// runtime (vastleggen gaat niet over HTTP).
fn de_cel(
    definitie: &ProcesDefinitie,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> Result<Arc<Cell>, Vec<String>> {
    let mut genoemd: Vec<(String, &str)> = Vec::new();
    if let Some(p) = &definitie.portal {
        genoemd.push(("portaal".into(), &p.cell));
    }
    if let Some(b) = &definitie.handling {
        genoemd.push(("behandeling.werkvoorraad".into(), &b.worklist.cell));
        for h in &b.actions {
            genoemd.push((
                format!("handeling '{}', vastleggen", h.name),
                &h.record.cell,
            ));
        }
    }
    for b in definitie.zaakbronnen() {
        genoemd.push((
            format!("synthese-bron {}/{}", b.cell, b.lexostatus),
            &b.cell,
        ));
    }
    let Some((_, eerste)) = genoemd.first() else {
        return Err(vec![
            "het proces noemt geen cell: zonder portaal en zonder behandeling laat het nergens vastleggen"
                .into(),
        ]);
    };
    let mut fouten = Vec::new();
    for (waar, cell) in &genoemd {
        if cell != eerste {
            fouten.push(format!(
                "{waar}: cel '{cell}', en het proces legt vast in cel '{eerste}'; een proces handelt over de zaken van een cel"
            ));
        }
    }
    match cells.get(*eerste) {
        Some(c) if fouten.is_empty() => Ok(c.clone()),
        Some(_) => Err(fouten),
        None => {
            fouten.push(format!(
                "cel '{eerste}' draait niet in deze runtime; een proces laat alleen vastleggen in een cel van dezelfde runtime"
            ));
            Err(fouten)
        }
    }
}

/// De actor van het proces is de `recording_actor` van elke stroom waarin
/// het vastlegt: die van het portaal en die van het besluit. Wat een stroom
/// niet noemt, bestaat niet: dat meldt de controle op portaal en besluit.
fn actor_legt_vast(definitie: &ProcesDefinitie, cell: &Cell) -> Vec<String> {
    let mut streams: Vec<(String, &str)> = Vec::new();
    if let Some(p) = &definitie.portal {
        streams.push(("portaal".into(), &p.stream));
    }
    for h in definitie.handling.iter().flat_map(|b| b.actions.iter()) {
        streams.push((
            format!("handeling '{}', vastleggen", h.name),
            &h.record.stream,
        ));
    }
    let mut fouten = Vec::new();
    for (waar, id) in streams {
        if let Some(s) = cell.streams.iter().find(|s| s.id == id) {
            if s.recording_actor != definitie.actor {
                fouten.push(format!(
                    "{waar}: stroom '{id}' heeft recording_actor '{}', en het proces handelt als '{}'",
                    s.recording_actor, definitie.actor
                ));
            }
        }
    }
    fouten
}

/// Een voorbeeld voor een handeling die het proces niet heeft, is een fout:
/// logins zonder rollen, een aanvraag zonder portaal, een formulier voor een
/// handeling die er niet is. (Dat een portaal een rol heeft die het mag,
/// controleert [`crate::kanaal::controleer_proces`].)
fn voorbeelden_zonder_handeling(
    definitie: &ProcesDefinitie,
    v: &VoorbeeldenDefinitie,
) -> Vec<String> {
    let mut fouten = Vec::new();
    if !v.logins.is_empty() && definitie.roles.is_empty() {
        fouten.push("voorbeelden.inloggen: het proces heeft geen rollen".to_string());
    }
    if v.application.is_some() && definitie.portal.is_none() {
        fouten.push("voorbeelden.aanvraag: het proces heeft geen portaal".to_string());
    }
    for name in v.actions.keys() {
        if definitie
            .handling
            .as_ref()
            .and_then(|b| b.action(name))
            .is_none()
        {
            fouten.push(format!(
                "voorbeelden.handelingen: het proces heeft geen handeling '{name}'"
            ));
        }
    }
    fouten
}

/// Zet het proces voor elke melding.
pub fn met_proces(proces: &str, fouten: Vec<String>) -> Vec<String> {
    fouten
        .into_iter()
        .map(|f| format!("proces '{proces}': {f}"))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn service() -> Arc<LawExecutionService> {
        Arc::new(
            crate::regelingen::laad(&fixtures().join("regulation"))
                .unwrap()
                .service,
        )
    }

    fn cells(s: &Arc<LawExecutionService>) -> BTreeMap<String, Arc<Cell>> {
        crate::config::celmappen(&fixtures().join("cells"))
            .unwrap()
            .iter()
            .map(|m| {
                let c = Cell::laad(m, s.clone()).unwrap();
                (c.id().to_string(), Arc::new(c))
            })
            .collect()
    }

    /// Kopieer een fixture-proces naar een tijdelijke map, met een aanpassing
    /// aan zijn `proces.yaml`.
    fn met(proces: &str, pas_aan: impl Fn(String) -> String) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for e in std::fs::read_dir(fixtures().join("processes").join(proces)).unwrap() {
            let p = e.unwrap().path();
            let mut tekst = std::fs::read_to_string(&p).unwrap();
            if p.file_name().unwrap() == "process.yaml" {
                tekst = pas_aan(tekst);
            }
            std::fs::write(dir.path().join(p.file_name().unwrap()), tekst).unwrap();
        }
        dir
    }

    fn fouten(proces: &str, pas_aan: impl Fn(String) -> String) -> Vec<String> {
        let s = service();
        let dir = met(proces, pas_aan);
        Proces::laad(dir.path(), &cells(&s), s).err().unwrap()
    }

    #[test]
    fn fixture_processen_laden() {
        let s = service();
        let c = cells(&s);
        let instantie =
            Proces::laad(&fixtures().join("processes/instantie"), &c, s.clone()).unwrap();
        assert_eq!(instantie.cell.id(), "test_instantie");
        assert_eq!(
            instantie.portaal_event().unwrap().1.name,
            "aanvraag_ontvangen"
        );
        assert!(instantie.form.is_some());
        let afnemer = Proces::laad(&fixtures().join("processes/afnemer"), &c, s).unwrap();
        assert_eq!(afnemer.cell.id(), "test_afnemer");
        assert_eq!(afnemer.definitie.zaakbronnen().count(), 3);
        assert_eq!(afnemer.definitie.andere_bronnen().count(), 2);
        // Wat bij het besluit nog niet gebeurd is, staat niet in proces.yaml:
        // het volgt uit de procedure van de beschikking (stage BEKENDMAKING na
        // BESLUIT). De dag van bekendmaking leidt de lexostatus besluit af
        // (geen gram: leeg), dus die staat er niet bij.
        let b = afnemer.definitie.handling.as_ref().unwrap();
        let decision = b.action("besluit").unwrap();
        assert_eq!(decision.soort, crate::config::Handelingsoort::Decision);
        assert_eq!(decision.stage.as_deref(), Some("BESLUIT"));
        let stand = &decision.not_yet;
        assert_eq!(stand["bekendgemaakt"].value, serde_json::json!(false));
        assert_eq!(stand["bekendgemaakt"].stage, "BEKENDMAKING");
        assert_eq!(stand.len(), 1);
        // De bekendmaking is een vervolg op het besluit, met de haak van de
        // bezwaartermijn; de betaling een feit met een toets.
        let bekend = b.action("bekendmaken").unwrap();
        assert_eq!(
            bekend.soort,
            crate::config::Handelingsoort::FollowUp {
                decision: "besluit".into(),
                procedure: "beschikking".into()
            }
        );
        assert_eq!(bekend.hooks, ["testregeling_awb#4"]);
        assert_eq!(
            bekend.outputs,
            [
                "besluit_tijdig",
                "aanvang_bezwaartermijn",
                "einde_bezwaartermijn"
            ]
        );
        let betalen = b.action("betalen").unwrap();
        assert_eq!(betalen.soort, crate::config::Handelingsoort::Fact);
        assert_eq!(betalen.assessments, ["betaling_conform"]);
    }

    /// Een vertaling in de synthese rust op een grondslag: elke grondslag
    /// wijst een geladen artikel aan, en met `herkomst: streng` heeft elke
    /// bron die vertaalt er een. De fixture vertaalt met grondslag bij het
    /// register, en zonder bij de registerstatus.
    #[test]
    fn de_grondslag_van_een_vertaling() {
        let streng = |t: String| {
            t.replace(
                "actor: test_afnemer\n",
                "actor: test_afnemer\norigin_check: strict\n",
            )
        };
        let f = fouten("afnemer", streng);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].contains("synthese-bron test_register/registerstatus: vertaalt (")
                && f[0].contains("geblokkeerd -> geblokkeerd_raad")
                && f[0].contains("zonder grondslag"),
            "{f:?}"
        );
        // Een grondslag die niet bestaat, ook buiten streng.
        let f = fouten("afnemer", |t| {
            t.replace(
                "legal_basis: [testregeling_afnemer#1, testregeling_register#1]",
                "legal_basis: [testregeling_afnemer#1, testregeling_register#9]",
            )
        });
        assert_eq!(
            f,
            ["proces 'test_afnemer_proces': synthese-bron test_register/register: grondslag 'testregeling_register#9': regeling 'testregeling_register' heeft geen artikel 9"]
        );
        // Een vaste waarde in de invoer van een bron per regel is ook een
        // vertaling.
        let f = fouten("afnemer", |t| {
            streng(t).replace(
                "gebied: {column: gebied}\n                peildatum",
                "gebied: {value: noord}\n                peildatum",
            )
        });
        assert!(
            f.iter().any(|m| m.contains("handeling 'besluit', rijen 'gebiedstabel', bron test_gebieden/tarief: vertaalt (gebied = \"noord\") zonder grondslag")),
            "{f:?}"
        );
    }

    /// De grondslag van een kanaal en van een veld wijst een geladen artikel
    /// aan, met het lid dat ze noemt.
    #[test]
    fn de_grondslag_van_een_kanaal() {
        let f = fouten("afnemer", |t| {
            t.replace(
                "legal_basis: [testregeling_afnemer#1]",
                "legal_basis: [testregeling_afnemer#1 lid 4]",
            )
        });
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].contains("kanaal 'eherkenning': grondslag 'testregeling_afnemer#1 lid 4': artikel 1 heeft geen lid 4"),
            "{f:?}"
        );
        let f = fouten("afnemer", |t| {
            t.replace(
                "legal_basis: [testregeling_register#1]",
                "legal_basis: [testregeling_onbekend#1]",
            )
        });
        assert_eq!(
            f,
            ["proces 'test_afnemer_proces': kanaal 'eherkenning', veld 'kvk': grondslag 'testregeling_onbekend#1': regeling 'testregeling_onbekend' is niet geladen"]
        );
    }

    #[test]
    fn elke_melding_noemt_het_proces() {
        let f = fouten("instantie", |t| {
            t.replace("cell: test_instantie", "cell: bestaat_niet")
        });
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].starts_with(
                "proces 'test_instantie_proces': cel 'bestaat_niet' draait niet in deze runtime"
            ),
            "{f:?}"
        );
    }

    #[test]
    fn een_proces_handelt_in_een_cel() {
        let f = fouten("afnemer", |t| {
            t.replace(
                "worklist: {cell: test_afnemer,",
                "worklist: {cell: test_register,",
            )
        });
        assert!(
            f.iter().any(|f| f.contains(
                "behandeling.werkvoorraad: cel 'test_register', en het proces legt vast in cel 'test_afnemer'"
            )),
            "{f:?}"
        );
    }

    #[test]
    fn een_proces_zonder_cel() {
        let f = fouten("instantie", |t| {
            t.split("\nroles:").next().unwrap().to_string()
        });
        assert!(f[0].contains("het proces noemt geen cel"), "{f:?}");
    }

    #[test]
    fn de_actor_legt_vast_in_zijn_eigen_stromen() {
        let f = fouten("afnemer", |t| {
            t.replace("actor: test_afnemer", "actor: iemand_anders")
        });
        assert!(
            f.iter().any(|f| f.contains(
                "portaal: stroom 'test_afnemer_aanvragen' heeft recording_actor 'test_afnemer', en het proces handelt als 'iemand_anders'"
            )),
            "{f:?}"
        );
        assert!(
            f.iter().any(|f| f
                .contains("handeling 'besluit', vastleggen: stroom 'test_afnemer_zaakverloop'")),
            "{f:?}"
        );
    }

    #[test]
    fn portaal_met_onbekend_event() {
        let f = fouten("instantie", |t| {
            t.replace("event: aanvraag_ontvangen", "event: bestaat_niet")
        });
        assert!(
            f.iter()
                .any(|f| f.contains("stroom 'test_aanvragen' heeft geen event 'bestaat_niet'")),
            "{f:?}"
        );
    }

    #[test]
    fn voorbeelden_van_de_fixture() {
        let s = service();
        let afnemer = Proces::laad(&fixtures().join("processes/afnemer"), &cells(&s), s).unwrap();
        let labels: Vec<&str> = afnemer
            .examples
            .logins
            .iter()
            .map(|v| v.label.as_str())
            .collect();
        assert_eq!(labels, ["voorbeeld-login", "voorbeeld-login-ander"]);
        assert!(afnemer.examples.application.is_some());
        assert!(afnemer.examples.actions.contains_key("besluit"));
    }

    #[test]
    fn voorbeeld_voor_een_handeling_die_het_proces_niet_heeft() {
        let f = fouten("instantie", |t| {
            format!("{t}\nexamples:\n  actions: {{besluit: weg.json}}\n")
        });
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("voorbeelden.handelingen: het proces heeft geen handeling 'besluit'"));
        assert!(f[1].contains("weg.json"), "{f:?}");
    }
}
