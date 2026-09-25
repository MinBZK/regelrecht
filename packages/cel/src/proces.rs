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

use crate::cel::Cel;
use crate::config::{Portaal, ProcesDefinitie, RijenDefinitie, VoorbeeldenDefinitie};
use crate::controle;
use crate::formulier::{self, Formulier};
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
    pub cel: Arc<Cel>,
    /// Het corpus, gedeeld door de hele runtime.
    pub service: Arc<LawExecutionService>,
    pub formulier: Option<Formulier>,
    /// Standaardgegevens per handeling (leeg zonder `voorbeelden`).
    pub voorbeelden: Voorbeelden,
    /// Wat de controle op de herkomst van de parameters zag, maar geen reden
    /// is om niet te starten (zie [`crate::origin`]).
    pub waarschuwingen: Vec<String>,
    /// Het tijdvak dat het portaal laat kiezen, als het aanbod er een vraagt.
    pub tijdvak: Option<Tijdvak>,
}

/// Het tijdvak van het aanbod: de parameter met origin BELANGHEBBENDE en
/// grondslag Awb 4:2 lid 1 (zie [`crate::origin`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Tijdvak {
    pub parameter: String,
    /// Het veld van het concept (`external`) waaruit de toets-lexostatus de
    /// parameter afleidt, als ze dat doet. Het portaal vult het vooraf in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veld: Option<String>,
}

impl Proces {
    /// Laad een proces uit zijn map en controleer het tegen de cellen van de
    /// runtime. Elke fout komt terug, en elke fout noemt het proces. De
    /// controles op synthese en besluit staan in [`crate::synthese::controleer`]
    /// en [`crate::besluit::controleer`]; de runtime roept ze aan.
    pub fn laad(
        map: &Path,
        cellen: &BTreeMap<String, Arc<Cel>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let naam = map
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let definitie = ProcesDefinitie::laad(map).map_err(|f| met_proces(&naam, f))?;
        let id = definitie.id.clone();
        Self::laad_definitie(definitie, map, cellen, service).map_err(|f| met_proces(&id, f))
    }

    fn laad_definitie(
        mut definitie: ProcesDefinitie,
        map: &Path,
        cellen: &BTreeMap<String, Arc<Cel>>,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let cel = de_cel(&definitie, cellen)?;
        let mut fouten = Vec::new();
        fouten.extend(actor_legt_vast(&definitie, &cel));
        if let Some(p) = &definitie.portaal {
            fouten.extend(controle::portaal(
                &cel.strommen,
                &cel.lexostatussen,
                p,
                &service,
            ));
            for r in &p.toets.rijen {
                fouten.extend(rijen::controleer(
                    "toets",
                    r,
                    &[p.toets.lexostatus.as_str()],
                    "niet de toets-lexostatus",
                    &definitie,
                    &cel,
                ));
            }
        }
        fouten.extend(vind_besluit(&mut definitie, &service));
        if let Err(f) = crate::besluit::zet_stand_bij_besluit(&mut definitie, &service, &cel) {
            fouten.push(f);
        }
        let formulier = match definitie
            .portaal
            .as_ref()
            .and_then(|p| p.formulier.as_ref())
        {
            Some(f) => formulier::laad(&map.join(&f.pad), &f.scherm)
                .map_err(|e| fouten.push(e))
                .ok(),
            None => None,
        };
        // Elke grondslag in het formulier wijst een geladen artikel aan, en
        // een lid dat het artikel heeft.
        for (waar, g) in formulier.iter().flat_map(Formulier::grondslagen) {
            if let Err(f) = crate::regelingen::geldig(&service, &g) {
                fouten.push(format!("formulier, {waar}: {f}"));
            }
        }
        let voorbeelden = match &definitie.voorbeelden {
            Some(v) => {
                fouten.extend(voorbeelden_zonder_handeling(&definitie, v));
                voorbeelden::laad(map, v)
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
            cel,
            service,
            formulier,
            voorbeelden,
            waarschuwingen: Vec::new(),
            tijdvak: None,
        })
    }

    pub fn id(&self) -> &str {
        &self.definitie.id
    }

    /// De controle op de herkomst van de parameters (zie [`crate::origin`]).
    /// Het besluitformulier volgt eruit, en de waarschuwingen bewaart het
    /// proces; de fouten komen terug.
    pub fn controleer_herkomst(&mut self, cellen: &BTreeMap<String, Arc<Cel>>) -> Vec<String> {
        let c = origin::controleer(&self.definitie, &self.cel, cellen, &self.service);
        let oordelen = origin::oordelen(&c, &self.service);
        if let Some(b) = self.definitie.behandeling.as_mut() {
            b.besluit.formulier = oordelen;
        }
        self.waarschuwingen = c.waarschuwingen;
        self.tijdvak = c.tijdvak.map(|parameter| Tijdvak {
            veld: self.concept_veld(&parameter),
            parameter,
        });
        c.fouten
    }

    /// Het veld van het concept dat de toets-lexostatus leest om een
    /// parameter af te leiden: een `$external`-sleutel van het portaal-event.
    fn concept_veld(&self, parameter: &str) -> Option<String> {
        let p = self.portaal()?;
        let (_, event) = self.portaal_event()?;
        let afleiding = self
            .cel
            .lexostatussen
            .lexostatus(&p.toets.lexostatus)?
            .reduction
            .afleidingen
            .get(parameter)?;
        let [pad] = afleiding.gelezen_paden()[..] else {
            return None;
        };
        event.bladeren().into_iter().find_map(|b| match b.binding {
            Binding::External(sleutel) if b.pad == pad && !sleutel.contains('.') => Some(sleutel),
            _ => None,
        })
    }

    /// Het portaalblok, als het proces een portaal heeft.
    pub fn portaal(&self) -> Option<&Portaal> {
        self.definitie.portaal.as_ref()
    }

    /// De stroom en het event waarin het portaal laat vastleggen.
    pub fn portaal_event(&self) -> Option<(&Stroom, &Event)> {
        let p = self.portaal()?;
        self.cel.event(&p.stroom, &p.event)
    }

    /// De rijen-definities van de toets (synthese per regel).
    pub fn toets_rijen(&self) -> &[RijenDefinitie] {
        self.portaal()
            .map(|p| p.toets.rijen.as_slice())
            .unwrap_or_default()
    }

    /// De rijen-definities van het besluit (synthese per regel).
    pub fn rijen(&self) -> &[RijenDefinitie] {
        self.definitie
            .behandeling
            .as_ref()
            .map(|b| b.besluit.rijen.as_slice())
            .unwrap_or_default()
    }
}

/// De cel waarin het proces vastlegt. Het portaal, de werkvoorraad, het
/// besluit en de bronnen van de zaak noemen dezelfde cel: in deze stap
/// handelt een proces over de zaken van een cel, en die cel draait in deze
/// runtime (vastleggen gaat niet over HTTP).
fn de_cel(
    definitie: &ProcesDefinitie,
    cellen: &BTreeMap<String, Arc<Cel>>,
) -> Result<Arc<Cel>, Vec<String>> {
    let mut genoemd: Vec<(String, &str)> = Vec::new();
    if let Some(p) = &definitie.portaal {
        genoemd.push(("portaal".into(), &p.cel));
    }
    if let Some(b) = &definitie.behandeling {
        genoemd.push(("behandeling.werkvoorraad".into(), &b.werkvoorraad.cel));
        if let Some(v) = &b.besluit.vastleggen {
            genoemd.push(("besluit.vastleggen".into(), &v.cel));
        }
    }
    for b in definitie.zaakbronnen() {
        genoemd.push((format!("synthese-bron {}/{}", b.cel, b.lexostatus), &b.cel));
    }
    let Some((_, eerste)) = genoemd.first() else {
        return Err(vec![
            "het proces noemt geen cel: zonder portaal en zonder behandeling laat het nergens vastleggen"
                .into(),
        ]);
    };
    let mut fouten = Vec::new();
    for (waar, cel) in &genoemd {
        if cel != eerste {
            fouten.push(format!(
                "{waar}: cel '{cel}', en het proces legt vast in cel '{eerste}'; een proces handelt over de zaken van een cel"
            ));
        }
    }
    match cellen.get(*eerste) {
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
fn actor_legt_vast(definitie: &ProcesDefinitie, cel: &Cel) -> Vec<String> {
    let mut stromen: Vec<(&str, &str)> = Vec::new();
    if let Some(p) = &definitie.portaal {
        stromen.push(("portaal", &p.stroom));
    }
    if let Some(v) = definitie
        .behandeling
        .as_ref()
        .and_then(|b| b.besluit.vastleggen.as_ref())
    {
        stromen.push(("besluit, vastleggen", &v.stroom));
    }
    let mut fouten = Vec::new();
    for (waar, id) in stromen {
        if let Some(s) = cel.strommen.iter().find(|s| s.id == id) {
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
/// logins of een aanvraag zonder portaal, een besluit zonder behandeling.
/// (Dat een portaal de rol aanvrager heeft, controleert
/// [`crate::besluit::controleer`].)
fn voorbeelden_zonder_handeling(
    definitie: &ProcesDefinitie,
    v: &VoorbeeldenDefinitie,
) -> Vec<String> {
    let mut fouten = Vec::new();
    if !v.inloggen.is_empty() && definitie.portaal.is_none() {
        fouten.push("voorbeelden.inloggen: het proces heeft geen portaal".to_string());
    }
    if v.aanvraag.is_some() && definitie.portaal.is_none() {
        fouten.push("voorbeelden.aanvraag: het proces heeft geen portaal".to_string());
    }
    if v.besluit.is_some() && definitie.behandeling.is_none() {
        fouten.push("voorbeelden.besluit: het proces heeft geen behandeling".to_string());
    }
    fouten
}

/// Vul de regeling van het besluit in uit de wet, als `proces.yaml` haar niet
/// noemt: de beschikking waarvoor het bevoegd gezag de `actor` van het proces
/// is. Precies een zo'n beschikking, en de uitkomsten van het besluit komen
/// uit dat artikel; anders een fout die de kandidaten noemt.
fn vind_besluit(definitie: &mut ProcesDefinitie, service: &LawExecutionService) -> Vec<String> {
    let actor = definitie.actor.clone();
    let Some(b) = definitie.behandeling.as_mut().map(|b| &mut b.besluit) else {
        return Vec::new();
    };
    if !b.regeling.is_empty() {
        return Vec::new();
    }
    let kandidaten = crate::besluit::beschikkingen_van(service, &actor);
    let [(regeling, artikel)] = kandidaten.as_slice() else {
        let lijst: Vec<String> = kandidaten.iter().map(|(r, a)| format!("{r}#{a}")).collect();
        return vec![if lijst.is_empty() {
            format!(
                "besluit: geen regeling noemt '{actor}' als bevoegd gezag bij een BESCHIKKING; noem de regeling in behandeling.besluit.regeling"
            )
        } else {
            format!(
                "besluit: '{actor}' is bevoegd voor meer dan een beschikking ({}); kies er een met behandeling.besluit.regeling",
                lijst.join(", ")
            )
        }];
    };
    let mut fouten = Vec::new();
    for u in &b.uitkomsten {
        let van = service
            .resolver()
            .get_article_by_output(regeling, u, None)
            .map(|a| a.number.clone());
        if van.as_deref() != Some(artikel.as_str()) {
            fouten.push(format!(
                "besluit: uitkomst '{u}' komt niet uit {regeling}#{artikel}, de beschikking waarvoor '{actor}' bevoegd is"
            ));
        }
    }
    b.regeling = regeling.clone();
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

    fn cellen(s: &Arc<LawExecutionService>) -> BTreeMap<String, Arc<Cel>> {
        crate::config::celmappen(&fixtures().join("cellen"))
            .unwrap()
            .iter()
            .map(|m| {
                let c = Cel::laad(m, s.clone()).unwrap();
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
            if p.file_name().unwrap() == "proces.yaml" {
                tekst = pas_aan(tekst);
            }
            std::fs::write(dir.path().join(p.file_name().unwrap()), tekst).unwrap();
        }
        dir
    }

    fn fouten(proces: &str, pas_aan: impl Fn(String) -> String) -> Vec<String> {
        let s = service();
        let dir = met(proces, pas_aan);
        Proces::laad(dir.path(), &cellen(&s), s).err().unwrap()
    }

    #[test]
    fn fixture_processen_laden() {
        let s = service();
        let c = cellen(&s);
        let instantie =
            Proces::laad(&fixtures().join("processes/instantie"), &c, s.clone()).unwrap();
        assert_eq!(instantie.cel.id(), "test_instantie");
        assert_eq!(
            instantie.portaal_event().unwrap().1.name,
            "aanvraag_ontvangen"
        );
        assert!(instantie.formulier.is_some());
        let afnemer = Proces::laad(&fixtures().join("processes/afnemer"), &c, s).unwrap();
        assert_eq!(afnemer.cel.id(), "test_afnemer");
        assert_eq!(afnemer.definitie.zaakbronnen().count(), 2);
        assert_eq!(afnemer.definitie.andere_bronnen().count(), 2);
        // De stand bij besluit staat niet in proces.yaml: ze volgt uit de
        // procedure van de beschikking (stage BEKENDMAKING na BESLUIT).
        let stand = &afnemer
            .definitie
            .behandeling
            .as_ref()
            .unwrap()
            .besluit
            .stand_bij_besluit;
        assert_eq!(stand["bekendgemaakt"].waarde, serde_json::json!(false));
        assert_eq!(stand["datum_bekendmaking"].waarde, serde_json::Value::Null);
        assert_eq!(stand["datum_bekendmaking"].stage, "BEKENDMAKING");
        assert_eq!(stand.len(), 2);
    }

    #[test]
    fn elke_melding_noemt_het_proces() {
        let f = fouten("instantie", |t| {
            t.replace("cel: test_instantie", "cel: bestaat_niet")
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
                "werkvoorraad: {cel: test_afnemer,",
                "werkvoorraad: {cel: test_register,",
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
            t.split("\nrollen:").next().unwrap().to_string()
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
            f.iter()
                .any(|f| f.contains("besluit, vastleggen: stroom 'test_afnemer_zaakverloop'")),
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
        let afnemer = Proces::laad(&fixtures().join("processes/afnemer"), &cellen(&s), s).unwrap();
        let labels: Vec<&str> = afnemer
            .voorbeelden
            .inloggen
            .iter()
            .map(|v| v.label.as_str())
            .collect();
        assert_eq!(labels, ["voorbeeld-login", "voorbeeld-login-ander"]);
        assert!(afnemer.voorbeelden.aanvraag.is_some());
        assert!(afnemer.voorbeelden.besluit.is_some());
    }

    #[test]
    fn voorbeeld_voor_een_handeling_die_het_proces_niet_heeft() {
        let f = fouten("instantie", |t| {
            format!("{t}\nvoorbeelden:\n  besluit: weg.json\n")
        });
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("voorbeelden.besluit: het proces heeft geen behandeling"));
        assert!(f[1].contains("weg.json"), "{f:?}");
    }
}
