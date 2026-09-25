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
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

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
    /// Een schrijver tegelijk, zodat regels niet door elkaar lopen en een
    /// controle ziet wat er werkelijk ligt. Lezers wachten niet op hem: de
    /// schijf (fsync) gebeurt zonder `staat` vast te houden, en alleen
    /// schrijvers veranderen `staat`.
    schrijver: Mutex<()>,
    /// Per kroniek de grammen.
    staat: RwLock<BTreeMap<String, Stapel>>,
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
            schrijver: Mutex::new(()),
            staat: RwLock::new(BTreeMap::new()),
        };
        k.laad(kronieken)?;
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

    // Een slot dat vergiftigd is (een draad paniekte eronder) blijft
    // bruikbaar: `staat` verandert alleen na een geslaagde schrijfactie, in
    // een stap, dus er is nooit iets half bijgewerkt.
    fn schrijfslot(&self) -> MutexGuard<'_, ()> {
        self.schrijver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lees_staat(&self) -> RwLockReadGuard<'_, BTreeMap<String, Stapel>> {
        self.staat.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn schrijf_staat(&self) -> RwLockWriteGuard<'_, BTreeMap<String, Stapel>> {
        self.staat.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Zorg dat deze kronieken in het geheugen staan; een kroniek die er nog
    /// niet is, wordt van schijf gelezen.
    fn laad(&self, kronieken: &[&str]) -> Result<(), String> {
        let ontbreekt: Vec<&str> = {
            let staat = self.lees_staat();
            kronieken
                .iter()
                .copied()
                .filter(|k| !staat.contains_key(*k))
                .collect()
        };
        if ontbreekt.is_empty() {
            return Ok(());
        }
        // Onder het schrijfslot, zodat niemand tegelijk aan het bestand
        // schrijft terwijl het gelezen (en zo nodig afgekapt) wordt.
        let _schrijver = self.schrijfslot();
        for k in ontbreekt {
            if self.lees_staat().contains_key(k) {
                continue;
            }
            let s = lees_bestand(&self.bestand(k)?)?;
            self.schrijf_staat().insert(k.to_string(), s);
        }
        Ok(())
    }

    /// Lees uit de stapels van `kronieken`, geladen.
    fn met_stapels<T>(
        &self,
        kronieken: &[&str],
        f: impl FnOnce(Vec<&Stapel>) -> T,
    ) -> Result<T, String> {
        self.laad(kronieken)?;
        let staat = self.lees_staat();
        let mut stapels = Vec::with_capacity(kronieken.len());
        for k in kronieken {
            stapels.push(
                staat
                    .get(*k)
                    .ok_or_else(|| format!("kroniek '{k}' niet geladen"))?,
            );
        }
        Ok(f(stapels))
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
        let mut alle = kronieken.to_vec();
        alle.push(gram.chronicle.as_str());
        self.laad(&alle)?;
        let _schrijver = self.schrijfslot();
        // Zolang wij het schrijfslot hebben, verandert `staat` niet: de
        // controle ziet wat er ligt, ook nadat het leesslot weer los is.
        let (bestaand, lengte) = {
            let staat = self.lees_staat();
            let mut bestaand = Vec::new();
            for k in kronieken {
                if let Some(s) = staat.get(*k) {
                    bestaand.extend(s.grammen.iter().cloned());
                }
            }
            let lengte = staat.get(&gram.chronicle).map_or(0, |s| s.lengte);
            (bestaand, lengte)
        };
        let zicht: Vec<&Gram> = bestaand.iter().map(|v| &v.gram).collect();
        if let Err(w) = controle(&zicht) {
            return Ok(Err(w));
        }
        schrijf_regel(&pad, lengte, regel.as_bytes())?;
        let mut staat = self.schrijf_staat();
        let stapel = staat.entry(gram.chronicle.clone()).or_default();
        stapel.lengte = lengte + regel.len() as u64;
        stapel.voeg_toe(gram.clone());
        Ok(Ok(()))
    }

    /// Zet de startstand in de kroniek, als elke kroniek van `kronieken` leeg
    /// is; onwaar als er al iets lag. Elk bestand wordt in een keer geschreven
    /// (een tijdelijk bestand, dan hernoemd), zodat een onderbroken start
    /// geen half bestand achterlaat. Beslaat de startstand meer dan een
    /// kroniek, dan is dat per bestand, niet over de bestanden heen.
    pub fn zet_startstand(&self, kronieken: &[&str], grammen: &[Gram]) -> Result<bool, String> {
        let mut per_kroniek: BTreeMap<&str, String> = BTreeMap::new();
        for g in grammen {
            per_kroniek
                .entry(g.chronicle.as_str())
                .or_default()
                .push_str(&als_regel(g)?);
        }
        let mut alle: Vec<&str> = kronieken.to_vec();
        alle.extend(per_kroniek.keys());
        self.laad(&alle)?;
        let _schrijver = self.schrijfslot();
        {
            let staat = self.lees_staat();
            if alle
                .iter()
                .any(|k| staat.get(*k).is_some_and(|s| !s.grammen.is_empty()))
            {
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
        let mut staat = self.schrijf_staat();
        for ((tijdelijk, pad), (k, tekst)) in klaar.iter().zip(&per_kroniek) {
            std::fs::rename(tijdelijk, pad).map_err(|e| format!("{}: {e}", pad.display()))?;
            // Wat hernoemd is, staat ook in het geheugen, ook als een
            // volgende hernoeming mislukt.
            let stapel = staat.entry((*k).to_string()).or_default();
            stapel.lengte = tekst.len() as u64;
            for g in grammen.iter().filter(|g| g.chronicle == *k) {
                stapel.voeg_toe(g.clone());
            }
        }
        if let Ok(d) = std::fs::File::open(&self.map) {
            // De hernoeming zelf duurzaam maken; lukt dat niet, dan staat het
            // bestand er toch al.
            let _ = d.sync_all();
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
        self.met_stapels(kronieken, |stapels| {
            stapels
                .iter()
                .flat_map(|s| s.grammen.iter().cloned())
                .collect()
        })
    }

    /// De grammen van een zaak, over de gegeven kronieken, in de volgorde
    /// van vastleggen per kroniek.
    pub fn lees_zaak(
        &self,
        kronieken: &[&str],
        zaakkenmerk: &str,
    ) -> Result<Vec<Arc<Vastgelegd>>, String> {
        self.met_stapels(kronieken, |stapels| {
            let mut uit = Vec::new();
            for s in stapels {
                if let Some(posities) = s.per_zaak.get(zaakkenmerk) {
                    uit.extend(posities.iter().map(|&i| s.grammen[i].clone()));
                }
            }
            uit
        })
    }
}

/// Lees een kroniek van schijf. Een laatste regel zonder regeleinde is
/// onvolledig geschreven: die wordt afgekapt, met een melding, maar pas als
/// de rest leesbaar is. Een tijdelijk bestand van een onderbroken startstand
/// wordt weggehaald.
fn lees_bestand(pad: &Path) -> Result<Stapel, String> {
    let fout = |e: std::io::Error| format!("{}: {e}", pad.display());
    let tijdelijk = pad.with_extension("jsonl.nieuw");
    if tijdelijk.exists() {
        tracing::warn!(bestand = %tijdelijk.display(), "resten van een onderbroken startstand weggehaald");
        std::fs::remove_file(&tijdelijk).map_err(fout)?;
    }
    let bytes = match std::fs::read(pad) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Stapel::default()),
        Err(e) => return Err(fout(e)),
    };
    let heel = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let tekst = std::str::from_utf8(&bytes[..heel])
        .map_err(|e| format!("{}: geen UTF-8: {e}", pad.display()))?;
    let mut stapel = Stapel {
        lengte: heel as u64,
        ..Stapel::default()
    };
    let mut zonder_vastgelegd_op = 0_usize;
    for (i, regel) in tekst.lines().enumerate() {
        if regel.trim().is_empty() {
            continue;
        }
        let mut gram: Gram = serde_json::from_str(regel)
            .map_err(|e| format!("{} regel {}: {e}", pad.display(), i + 1))?;
        if gram.vul_vastgelegd_op() {
            zonder_vastgelegd_op += 1;
        }
        stapel.voeg_toe(gram);
    }
    if zonder_vastgelegd_op > 0 {
        tracing::warn!(
            kroniek = %pad.display(),
            grammen = zonder_vastgelegd_op,
            "grammen zonder vastgelegd_op (van voor dat veld): gelezen alsof ze op hun op_moment zijn vastgelegd"
        );
    }
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
    Ok(stapel)
}

/// Schrijf een regel achter de eerste `lengte` bytes van een kroniek, en
/// wacht tot hij op schijf staat. Wat daarna nog in het bestand stond (de
/// rest van een eerder mislukte schrijfactie) wordt eerst weggehaald, zodat
/// een regel nooit achter een halve regel komt.
fn schrijf_regel(pad: &Path, lengte: u64, regel: &[u8]) -> Result<(), String> {
    let fout = |e: std::io::Error| format!("{}: {e}", pad.display());
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(pad)
        .map_err(fout)?;
    f.set_len(lengte).map_err(fout)?;
    f.seek(SeekFrom::Start(lengte)).map_err(fout)?;
    f.write_all(regel)
        .and_then(|()| f.sync_data())
        .map_err(fout)
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
            handelende_actor: None,
            op_moment: "2025-03-12T10:14:03+01:00".into(),
            op_moment_grondslag: None,
            vastgelegd_op: "2025-03-12T10:14:05+01:00".into(),
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
    fn een_gram_zonder_vastgelegd_op_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = gram(Z1);
        g.vastgelegd_op = String::new();
        let f = k.voeg_toe(&g).unwrap_err();
        assert!(f.contains("vastgelegd_op"), "{f}");
        assert_eq!(aantal(&k), 0);
    }

    /// Een kroniek van voor `vastgelegd_op` laadt nog: het gram krijgt zijn
    /// `op_moment` als registratietijd. Een gram met het veld houdt het zijne.
    #[test]
    fn een_oud_gram_zonder_vastgelegd_op_laadt_met_zijn_op_moment() {
        let dir = tempfile::tempdir().unwrap();
        let mut oud = serde_json::to_value(gram(Z1)).unwrap();
        oud.as_object_mut().unwrap().remove("vastgelegd_op");
        let nieuw = serde_json::to_string(&gram(Z2)).unwrap();
        std::fs::write(
            dir.path().join("test_kroniek.jsonl"),
            format!("{oud}\n{nieuw}\n"),
        )
        .unwrap();
        let k = open(dir.path());
        let grammen = k.lees("test_kroniek").unwrap();
        assert_eq!(grammen[0].gram.vastgelegd_op, "2025-03-12T10:14:03+01:00");
        assert_eq!(grammen[1].gram.vastgelegd_op, "2025-03-12T10:14:05+01:00");
        grammen[0].gram.valideer().unwrap();
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
        // Ook niet als er daarbij een halve laatste regel staat: eerst lezen,
        // dan pas afkappen.
        let met_staart = format!("{g}\n{{\"kind\": \n{g}\n{{\"ki");
        std::fs::write(&pad, &met_staart).unwrap();
        assert!(Kroniek::open(dir.path(), K).is_err());
        assert_eq!(std::fs::read_to_string(&pad).unwrap(), met_staart);
    }

    #[test]
    fn een_rest_van_een_mislukte_schrijfactie_wordt_overschreven() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&gram(Z1)).unwrap();
        // Een eerdere schrijfactie liet een halve regel achter die niet
        // teruggezet kon worden; de kroniek weet nog de goede lengte.
        let pad = dir.path().join("test_kroniek.jsonl");
        let heel = std::fs::read_to_string(&pad).unwrap();
        std::fs::write(&pad, format!("{heel}{{\"half")).unwrap();
        k.voeg_toe(&gram(Z2)).unwrap();
        drop(k);
        let k = open(dir.path());
        assert_eq!(aantal(&k), 2);
    }

    #[test]
    fn resten_van_een_onderbroken_startstand_worden_opgeruimd() {
        let dir = tempfile::tempdir().unwrap();
        let rest = dir.path().join("test_kroniek.jsonl.nieuw");
        std::fs::write(&rest, "{\"half").unwrap();
        let k = open(dir.path());
        assert!(!rest.exists());
        assert!(k.zet_startstand(K, &[gram(Z1)]).unwrap());
        assert_eq!(aantal(&k), 1);
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
