//! De kroniek: append-only opslag van grammen, een JSON-regel per gram in
//! `DATA_DIR/<cel>/<chronicle>.jsonl`.
//!
//! Er is bewust geen pad om een gram te wijzigen of te verwijderen. Een
//! correctie of herstel is een nieuw gram.
//!
//! Het bestand is de bron; de grammen staan daarnaast in het geheugen, met
//! een index per zaak, zodat een reductie of een vraag naar een zaak het
//! bestand niet bij elke vraag opnieuw leest. Het geheugen groeit alleen,
//! onder hetzelfde slot als het schrijven: wat in het geheugen staat, staat
//! op schijf. Wie de runtime draait, schrijft dus niet zelf in het bestand.
//!
//! Een regel telt pas als ze met een regeleinde eindigt. Een onvolledige
//! laatste regel (de runtime stopte midden in het schrijven) wordt bij het
//! openen afgekapt en gemeld; zo'n gram was ook nooit bevestigd. Een
//! onleesbare regel daarvoor is geen schrijffout maar een kapotte kroniek,
//! en dan opent de kroniek niet.

use std::collections::{BTreeMap, HashMap};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::gram::Gram;

/// Een vastgelegd gram, met zijn YAML zodra iemand die vroeg: het gram
/// verandert niet meer, dus de YAML hoeft maar een keer gemaakt te worden.
#[derive(Debug)]
pub struct Vastgelegd {
    pub gram: Gram,
    yaml: OnceLock<String>,
}

impl Vastgelegd {
    fn nieuw(gram: Gram) -> Arc<Self> {
        Arc::new(Self {
            gram,
            yaml: OnceLock::new(),
        })
    }

    /// De YAML van het gram, gemaakt met `maak` als die er nog niet is.
    pub fn yaml(
        &self,
        maak: impl FnOnce(&Gram) -> Result<String, String>,
    ) -> Result<String, String> {
        if let Some(y) = self.yaml.get() {
            return Ok(y.clone());
        }
        let y = maak(&self.gram)?;
        Ok(self.yaml.get_or_init(|| y).clone())
    }
}

/// De grammen van een kroniek in het geheugen.
#[derive(Default)]
struct Stapel {
    grammen: Vec<Arc<Vastgelegd>>,
    /// zaakkenmerk -> posities in `grammen`, in de volgorde van vastleggen.
    per_zaak: HashMap<String, Vec<usize>>,
    /// De lengte van het bestand: tot hier staat er een hele regel.
    lengte: u64,
}

impl Stapel {
    fn voeg_toe(&mut self, gram: Gram) {
        if let Some(z) = &gram.zaakkenmerk {
            self.per_zaak
                .entry(z.clone())
                .or_default()
                .push(self.grammen.len());
        }
        self.grammen.push(Vastgelegd::nieuw(gram));
    }
}

pub struct Kroniek {
    map: PathBuf,
    /// Per kroniek de grammen. Een schrijver tegelijk, zodat regels niet
    /// door elkaar lopen en een controle ziet wat er werkelijk ligt.
    staat: Mutex<BTreeMap<String, Stapel>>,
}

/// Een regel als JSONL, met regeleinde.
fn als_regel(gram: &Gram) -> Result<String, String> {
    gram.valideer()
        .map_err(|f| format!("gram valideert niet: {}", f.join("; ")))?;
    let mut regel = serde_json::to_string(gram).map_err(|e| e.to_string())?;
    regel.push('\n');
    Ok(regel)
}

impl Kroniek {
    /// Open (en maak zo nodig) de map met kronieken, en lees `kronieken` in
    /// het geheugen. Een onvolledige laatste regel wordt hier afgekapt.
    pub fn open(map: &Path, kronieken: &[&str]) -> Result<Self, String> {
        std::fs::create_dir_all(map).map_err(|e| format!("{}: {e}", map.display()))?;
        let k = Self {
            map: map.to_path_buf(),
            staat: Mutex::new(BTreeMap::new()),
        };
        {
            let mut staat = k.slot()?;
            for c in kronieken {
                k.stapel(&mut staat, c)?;
            }
        }
        Ok(k)
    }

    fn bestand(&self, chronicle: &str) -> Result<PathBuf, String> {
        // Het schema staat alleen [a-z0-9_] toe; hier nogmaals, want dit
        // wordt een bestandsnaam.
        let geldig = !chronicle.is_empty()
            && chronicle
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !geldig {
            return Err(format!("ongeldige kroniek '{chronicle}'"));
        }
        Ok(self.map.join(format!("{chronicle}.jsonl")))
    }

    fn slot(&self) -> Result<MutexGuard<'_, BTreeMap<String, Stapel>>, String> {
        self.staat
            .lock()
            .map_err(|_| "kroniek vergrendeld: een eerdere schrijver brak af".to_string())
    }

    /// De stapel van een kroniek; bij de eerste vraag gelezen van schijf.
    fn stapel<'s>(
        &self,
        staat: &'s mut BTreeMap<String, Stapel>,
        chronicle: &str,
    ) -> Result<&'s mut Stapel, String> {
        if !staat.contains_key(chronicle) {
            let s = lees_bestand(&self.bestand(chronicle)?)?;
            staat.insert(chronicle.to_string(), s);
        }
        staat
            .get_mut(chronicle)
            .ok_or_else(|| format!("kroniek '{chronicle}' niet geladen"))
    }

    /// Voeg een gram toe. Het gram moet valideren tegen `gram.json`.
    pub fn voeg_toe(&self, gram: &Gram) -> Result<(), String> {
        self.voeg_toe_mits(gram, &[], |_| Ok::<(), std::convert::Infallible>(()))?
            .map_err(|never| match never {})
    }

    /// Voeg een gram toe als `controle` dat toelaat. De controle ziet de
    /// grammen van `kronieken` zoals ze op dat moment vastliggen, onder
    /// hetzelfde slot als het schrijven. Zo kunnen twee gelijktijdige
    /// verzoeken niet allebei door een controle komen die op het andere had
    /// moeten stuiten.
    ///
    /// De buitenste fout is een fout van de opslag; de binnenste is de
    /// weigering van de controle, en dan is er niets vastgelegd.
    pub fn voeg_toe_mits<E>(
        &self,
        gram: &Gram,
        kronieken: &[&str],
        controle: impl FnOnce(&[&Gram]) -> Result<(), E>,
    ) -> Result<Result<(), E>, String> {
        let regel = als_regel(gram)?;
        let pad = self.bestand(&gram.chronicle)?;
        let mut staat = self.slot()?;
        let mut bestaand = Vec::new();
        for k in kronieken {
            bestaand.extend(self.stapel(&mut staat, k)?.grammen.iter().cloned());
        }
        let zicht: Vec<&Gram> = bestaand.iter().map(|v| &v.gram).collect();
        if let Err(w) = controle(&zicht) {
            return Ok(Err(w));
        }
        let stapel = self.stapel(&mut staat, &gram.chronicle)?;
        schrijf_regel(&pad, stapel.lengte, regel.as_bytes())?;
        stapel.lengte += regel.len() as u64;
        stapel.voeg_toe(gram.clone());
        Ok(Ok(()))
    }

    /// Zet de startstand in de kroniek, als elke kroniek van `kronieken` leeg
    /// is; onwaar als er al iets lag. Elk bestand wordt in een keer geschreven
    /// (een tijdelijk bestand, dan hernoemd), zodat een onderbroken start
    /// geen halve startstand achterlaat.
    pub fn zet_startstand(&self, kronieken: &[&str], grammen: &[Gram]) -> Result<bool, String> {
        let mut per_kroniek: BTreeMap<&str, String> = BTreeMap::new();
        for g in grammen {
            per_kroniek
                .entry(g.chronicle.as_str())
                .or_default()
                .push_str(&als_regel(g)?);
        }
        let mut staat = self.slot()?;
        for k in kronieken.iter().chain(per_kroniek.keys()) {
            if !self.stapel(&mut staat, k)?.grammen.is_empty() {
                return Ok(false);
            }
        }
        // Eerst alles naast de kroniek, dan hernoemen: een hernoeming is
        // per bestand atomair.
        let mut klaar = Vec::new();
        for (k, tekst) in &per_kroniek {
            let pad = self.bestand(k)?;
            let tijdelijk = pad.with_extension("jsonl.nieuw");
            schrijf_bestand(&tijdelijk, tekst.as_bytes())?;
            klaar.push((tijdelijk, pad));
        }
        for (tijdelijk, pad) in &klaar {
            std::fs::rename(tijdelijk, pad).map_err(|e| format!("{}: {e}", pad.display()))?;
        }
        if let Ok(d) = std::fs::File::open(&self.map) {
            // De hernoeming zelf duurzaam maken; lukt dat niet, dan staat het
            // bestand er toch al.
            let _ = d.sync_all();
        }
        for (k, tekst) in &per_kroniek {
            let stapel = self.stapel(&mut staat, k)?;
            stapel.lengte = tekst.len() as u64;
        }
        for g in grammen {
            self.stapel(&mut staat, &g.chronicle)?.voeg_toe(g.clone());
        }
        Ok(true)
    }

    /// Alle grammen van een kroniek, in de volgorde van vastleggen.
    pub fn lees(&self, chronicle: &str) -> Result<Vec<Arc<Vastgelegd>>, String> {
        self.alle(&[chronicle])
    }

    /// Alle grammen van deze kronieken, per kroniek in de volgorde van
    /// vastleggen.
    pub fn alle(&self, kronieken: &[&str]) -> Result<Vec<Arc<Vastgelegd>>, String> {
        let mut staat = self.slot()?;
        let mut uit = Vec::new();
        for k in kronieken {
            uit.extend(self.stapel(&mut staat, k)?.grammen.iter().cloned());
        }
        Ok(uit)
    }

    /// De grammen van een zaak, over de gegeven kronieken, in de volgorde
    /// van vastleggen per kroniek.
    pub fn lees_zaak(
        &self,
        kronieken: &[&str],
        zaakkenmerk: &str,
    ) -> Result<Vec<Arc<Vastgelegd>>, String> {
        let mut staat = self.slot()?;
        let mut uit = Vec::new();
        for k in kronieken {
            let s = self.stapel(&mut staat, k)?;
            if let Some(posities) = s.per_zaak.get(zaakkenmerk) {
                uit.extend(posities.iter().map(|&i| s.grammen[i].clone()));
            }
        }
        Ok(uit)
    }
}

/// Lees een kroniek van schijf. Een laatste regel zonder regeleinde is
/// onvolledig geschreven: die wordt afgekapt, met een melding.
fn lees_bestand(pad: &Path) -> Result<Stapel, String> {
    let fout = |e: std::io::Error| format!("{}: {e}", pad.display());
    let bytes = match std::fs::read(pad) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Stapel::default()),
        Err(e) => return Err(fout(e)),
    };
    let heel = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    if heel < bytes.len() {
        tracing::warn!(
            kroniek = %pad.display(),
            bytes = bytes.len() - heel,
            "onvolledige laatste regel afgekapt: de runtime stopte tijdens het schrijven, en dat gram is nooit bevestigd"
        );
        let f = OpenOptions::new().write(true).open(pad).map_err(fout)?;
        f.set_len(heel as u64)
            .and_then(|()| f.sync_all())
            .map_err(fout)?;
    }
    let tekst = std::str::from_utf8(&bytes[..heel])
        .map_err(|e| format!("{}: geen UTF-8: {e}", pad.display()))?;
    let mut stapel = Stapel {
        lengte: heel as u64,
        ..Stapel::default()
    };
    for (i, regel) in tekst.lines().enumerate() {
        if regel.trim().is_empty() {
            continue;
        }
        let gram: Gram = serde_json::from_str(regel)
            .map_err(|e| format!("{} regel {}: {e}", pad.display(), i + 1))?;
        stapel.voeg_toe(gram);
    }
    Ok(stapel)
}

/// Schrijf een regel aan het einde van een kroniek van `lengte` bytes, en
/// wacht tot hij op schijf staat. Mislukt dat halverwege, dan wordt het
/// bestand teruggezet op `lengte`, zodat het volgende gram niet achter een
/// halve regel komt.
fn schrijf_regel(pad: &Path, lengte: u64, regel: &[u8]) -> Result<(), String> {
    let fout = |e: std::io::Error| format!("{}: {e}", pad.display());
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(pad)
        .map_err(fout)?;
    if let Err(e) = f.write_all(regel).and_then(|()| f.sync_data()) {
        if let Err(h) = f.set_len(lengte) {
            tracing::error!(kroniek = %pad.display(), "terugzetten na een mislukte schrijfactie mislukte ook: {h}");
        }
        return Err(fout(e));
    }
    Ok(())
}

/// Schrijf een heel bestand en wacht tot het op schijf staat.
fn schrijf_bestand(pad: &Path, inhoud: &[u8]) -> Result<(), String> {
    let fout = |e: std::io::Error| format!("{}: {e}", pad.display());
    let mut f = std::fs::File::create(pad).map_err(fout)?;
    f.write_all(inhoud)
        .and_then(|()| f.sync_all())
        .map_err(fout)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::StroomVerwijzing;
    use crate::stroom::Zaak;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn gram(zaak: &str) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            type_: "indiening".into(),
            soort: Some("melding".into()),
            stage: None,
            name: "melding_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            grondslag: vec!["testregeling_aanvraag#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            op_moment: "2025-03-12T10:14:03+01:00".into(),
            zaak: Zaak::Opent,
            zaakkenmerk: Some(zaak.into()),
            stroom: StroomVerwijzing {
                id: "test".into(),
                sha256: "a".repeat(64),
            },
            herkomst: None,
            fields: json!({"x": 1}).as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
        }
    }

    const Z1: &str = "00000000-0000-4000-8000-000000000001";
    const Z2: &str = "00000000-0000-4000-8000-000000000002";
    const K: &[&str] = &["test_kroniek"];

    fn open(dir: &Path) -> Kroniek {
        Kroniek::open(dir, K).unwrap()
    }

    fn aantal(k: &Kroniek) -> usize {
        k.lees("test_kroniek").unwrap().len()
    }

    #[test]
    fn toevoegen_en_lezen() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        assert_eq!(aantal(&k), 0);
        k.voeg_toe(&gram(Z1)).unwrap();
        k.voeg_toe(&gram(Z2)).unwrap();
        k.voeg_toe(&gram(Z1)).unwrap();
        assert_eq!(aantal(&k), 3);
        assert_eq!(k.lees_zaak(K, Z1).unwrap().len(), 2);
        // Na opnieuw openen staat hetzelfde er, uit het bestand.
        drop(k);
        let k = open(dir.path());
        assert_eq!(aantal(&k), 3);
        assert_eq!(k.lees_zaak(K, Z2).unwrap().len(), 1);
    }

    #[test]
    fn alleen_toevoegen_eerdere_regels_blijven_staan() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&gram(Z1)).unwrap();
        let pad = dir.path().join("test_kroniek.jsonl");
        let voor = std::fs::read_to_string(&pad).unwrap();
        k.voeg_toe(&gram(Z2)).unwrap();
        let na = std::fs::read_to_string(&pad).unwrap();
        assert!(na.starts_with(&voor));
        assert_eq!(na.lines().count(), 2);
    }

    #[test]
    fn ongeldig_gram_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = gram(Z1);
        g.zaakkenmerk = Some("geen-uuid".into());
        assert!(k.voeg_toe(&g).unwrap_err().contains("zaakkenmerk"));
        // Een event met een zaak zonder zaakkenmerk, en een zonder zaak met.
        g.zaakkenmerk = None;
        assert!(k.voeg_toe(&g).unwrap_err().contains("zaakkenmerk"));
        g.zaak = Zaak::Geen;
        g.zaakkenmerk = Some(Z1.into());
        assert!(k.voeg_toe(&g).is_err());
        assert_eq!(aantal(&k), 0);
    }

    #[test]
    fn een_gram_met_een_ongeldig_op_moment_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = gram(Z1);
        g.op_moment = "2025-03-12 10:14".into();
        let f = k.voeg_toe(&g).unwrap_err();
        assert!(f.contains("ongeldig op_moment '2025-03-12 10:14'"), "{f}");
        assert_eq!(aantal(&k), 0);
    }

    #[test]
    fn een_weigering_van_de_controle_legt_niets_vast() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&gram(Z1)).unwrap();
        let uitkomst = k
            .voeg_toe_mits(&gram(Z1), K, |bestaand| {
                if bestaand.is_empty() {
                    Ok(())
                } else {
                    Err("er ligt al iets")
                }
            })
            .unwrap();
        assert_eq!(uitkomst, Err("er ligt al iets"));
        assert_eq!(aantal(&k), 1);
    }

    #[test]
    fn gelijktijdige_controles_laten_er_een_door() {
        let dir = tempfile::tempdir().unwrap();
        let k = Arc::new(open(dir.path()));
        let draden: Vec<_> = (0..8)
            .map(|_| {
                let k = k.clone();
                std::thread::spawn(move || {
                    k.voeg_toe_mits(&gram(Z1), K, |bestaand| {
                        if bestaand.is_empty() {
                            Ok(())
                        } else {
                            Err(())
                        }
                    })
                    .unwrap()
                    .is_ok()
                })
            })
            .collect();
        let gelukt = draden
            .into_iter()
            .map(|d| d.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(gelukt, 1);
        assert_eq!(aantal(&k), 1);
        let pad = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(pad).unwrap().lines().count(), 1);
    }

    #[test]
    fn kroniek_naam_is_geen_pad() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        assert!(k.lees("../elders").is_err());
        assert!(Kroniek::open(dir.path(), &["../elders"]).is_err());
    }

    #[test]
    fn een_halve_laatste_regel_wordt_bij_het_openen_afgekapt() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&gram(Z1)).unwrap();
        drop(k);
        // De runtime stopte midden in het schrijven van het tweede gram.
        let pad = dir.path().join("test_kroniek.jsonl");
        let heel = std::fs::read_to_string(&pad).unwrap();
        let tweede = serde_json::to_string(&gram(Z2)).unwrap();
        std::fs::write(&pad, format!("{heel}{}", &tweede[..40])).unwrap();

        let k = open(dir.path());
        assert_eq!(aantal(&k), 1);
        assert_eq!(std::fs::read_to_string(&pad).unwrap(), heel);
        // En daarna gaat het vastleggen gewoon verder, op een hele regel.
        k.voeg_toe(&gram(Z2)).unwrap();
        drop(k);
        let k = open(dir.path());
        assert_eq!(aantal(&k), 2);
    }

    #[test]
    fn een_kapotte_regel_middenin_opent_niet() {
        let dir = tempfile::tempdir().unwrap();
        let pad = dir.path().join("test_kroniek.jsonl");
        let g = serde_json::to_string(&gram(Z1)).unwrap();
        std::fs::write(&pad, format!("{g}\n{{\"kind\": \n{g}\n")).unwrap();
        let fout = Kroniek::open(dir.path(), K).err().unwrap();
        assert!(fout.contains("test_kroniek.jsonl regel 2"), "{fout}");
        // Het bestand is niet aangeraakt.
        assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 3);
    }

    #[test]
    fn de_startstand_in_een_keer_en_alleen_in_een_lege_kroniek() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let stand = [gram(Z1), gram(Z2)];
        assert!(k.zet_startstand(K, &stand).unwrap());
        assert_eq!(aantal(&k), 2);
        let pad = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(&pad).unwrap().lines().count(), 2);
        assert!(!dir.path().join("test_kroniek.jsonl.nieuw").exists());
        // Niet nog eens.
        assert!(!k.zet_startstand(K, &stand).unwrap());
        assert_eq!(aantal(&k), 2);
        // Een ongeldig gram: niets geschreven.
        let leeg = tempfile::tempdir().unwrap();
        let k = open(leeg.path());
        let mut slecht = gram(Z2);
        slecht.op_moment = "gisteren".into();
        assert!(k.zet_startstand(K, &[gram(Z1), slecht]).is_err());
        assert_eq!(aantal(&k), 0);
        assert!(!leeg.path().join("test_kroniek.jsonl").exists());
    }

    #[test]
    fn de_yaml_wordt_een_keer_gemaakt() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&gram(Z1)).unwrap();
        let v = k.lees("test_kroniek").unwrap().remove(0);
        let mut keren = 0;
        for _ in 0..3 {
            let y = v
                .yaml(|g| {
                    keren += 1;
                    Ok(g.name.clone())
                })
                .unwrap();
            assert_eq!(y, "melding_ontvangen");
        }
        assert_eq!(keren, 1);
    }
}
