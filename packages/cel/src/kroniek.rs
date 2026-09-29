//! De kroniek: append-only opslag van grammen, een JSON-regel per gram in
//! `DATA_DIR/<cel>/<chronicle>.jsonl`.
//!
//! Er is bewust geen pad om een gram te wijzigen of te verwijderen. Een
//! correctie of herstel is een nieuw gram.
//!
//! Het bestand is de bron; de grammen staan daarnaast in het geheugen, met
//! een index op id en een index per wortel, zodat een reductie of een vraag
//! naar een groep het bestand niet bij elke vraag opnieuw leest. De wortel
//! van een gram is de wortel van het gram waarnaar het verwijst; een gram
//! zonder verwijzing is zijn eigen wortel (zie [`crate::gram`]). Het geheugen groeit alleen,
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

use chrono::{DateTime, FixedOffset};

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
    grams: Vec<Arc<Vastgelegd>>,
    /// De lengte van het bestand: tot hier staat er een hele regel.
    lengte: u64,
}

/// Alle geladen kronieken, met de indexen over de kronieken heen: een gram
/// mag verwijzen naar een gram in een andere kroniek van dezelfde cel.
#[derive(Default)]
struct Staat {
    stapels: BTreeMap<String, Stapel>,
    /// id -> het gram.
    per_id: HashMap<String, Arc<Vastgelegd>>,
    /// wortel -> de grammen met die wortel, in de volgorde van vastleggen.
    per_wortel: HashMap<String, Vec<Arc<Vastgelegd>>>,
}

impl Staat {
    /// Voeg een gram toe waarvan de wortel bekend is.
    fn voeg_toe(&mut self, gram: Gram) -> Arc<Vastgelegd> {
        let root = gram.root.clone().unwrap_or_else(|| gram.id.clone());
        let v = Vastgelegd::nieuw(gram);
        self.per_id.insert(v.gram.id.clone(), v.clone());
        self.per_wortel.entry(root).or_default().push(v.clone());
        self.stapels
            .entry(v.gram.chronicle.clone())
            .or_default()
            .grams
            .push(v.clone());
        v
    }

    /// De wortel van een gram dat naar `doelen` verwijst: die van de doelen,
    /// als ze er allemaal zijn en dezelfde wortel hebben. Zonder verwijzing
    /// zijn eigen id.
    fn wortel_van(&self, gram: &Gram, extra: &HashMap<String, String>) -> Root {
        if gram.refers_to.is_empty() {
            return Root::Own;
        }
        let mut gevonden: Option<String> = None;
        for id in gram.refers_to.values() {
            let w = match self.per_id.get(id) {
                Some(v) => v.gram.root.clone().unwrap_or_else(|| v.gram.id.clone()),
                None => match extra.get(id) {
                    Some(w) => w.clone(),
                    None => return Root::Onbekend(id.clone()),
                },
            };
            match &gevonden {
                Some(g) if *g != w => return Root::Verschillend,
                _ => gevonden = Some(w),
            }
        }
        gevonden.map_or(Root::Own, Root::Van)
    }

    /// De grammen met deze wortel in deze kronieken, in de volgorde van
    /// vastleggen.
    fn van_de_wortel(&self, chronicles: &[&str], root: &str) -> Vec<Arc<Vastgelegd>> {
        self.per_wortel
            .get(root)
            .into_iter()
            .flatten()
            .filter(|v| chronicles.contains(&v.gram.chronicle.as_str()))
            .cloned()
            .collect()
    }
}

/// Wat de wortel van een gram is.
enum Root {
    /// Het gram verwijst nergens naar: het is zijn eigen wortel.
    Own,
    /// De wortel van de grammen waarnaar het verwijst.
    Van(String),
    /// Het gram verwijst naar een id dat (nog) niet geladen is.
    Onbekend(String),
    /// De grammen waarnaar het verwijst, hebben verschillende wortels.
    Verschillend,
}

/// Wat een controle ziet, in eigen bezit (zie [`Kroniek::zicht_voor`]): de
/// grammen waarnaar een gram verwijst, de groep van zijn wortel en die
/// wortel.
pub struct ZichtEigen {
    doelen: Vec<Arc<Vastgelegd>>,
    group: Vec<Arc<Vastgelegd>>,
    pub root: Option<String>,
    wortelfout: Option<String>,
}

impl ZichtEigen {
    /// Het zicht om een controle mee te doen.
    pub fn zicht(&self) -> Zicht<'_> {
        Zicht {
            doelen: self
                .doelen
                .iter()
                .map(|v| (v.gram.id.clone(), &v.gram))
                .collect(),
            group: self.group.iter().map(|v| &v.gram).collect(),
            wortelfout: self.wortelfout.clone(),
        }
    }
}

impl Staat {
    /// Wat een controle over `gram` ziet, in `kronieken`.
    fn zicht(&self, chronicles: &[&str], gram: &Gram) -> ZichtEigen {
        let doelen: Vec<Arc<Vastgelegd>> = gram
            .refers_to
            .values()
            .filter_map(|id| self.per_id.get(id).cloned())
            .collect();
        let (root, wortelfout) = match self.wortel_van(gram, &HashMap::new()) {
            Root::Own => (None, None),
            Root::Van(w) => (Some(w), None),
            Root::Onbekend(id) => (None, Some(format!("geen gram '{id}' in de kroniek"))),
            Root::Verschillend => (
                None,
                Some("het gram verwijst naar grammen die niet bij dezelfde wortel horen".into()),
            ),
        };
        let group = root
            .as_deref()
            .map(|w| self.van_de_wortel(chronicles, w))
            .unwrap_or_default();
        ZichtEigen {
            doelen,
            group,
            root,
            wortelfout,
        }
    }
}

/// Wat een controle bij het vastleggen ziet (zie [`Kroniek::leg_vast_mits`]):
/// de grammen waarnaar het nieuwe gram verwijst, en de grammen van zijn
/// wortel, zoals ze op dat moment vastliggen.
pub struct Zicht<'a> {
    /// Per id waarnaar het gram verwijst het gram, als het er is.
    pub doelen: BTreeMap<String, &'a Gram>,
    /// De grammen met dezelfde wortel als het nieuwe gram, in de volgorde van
    /// vastleggen; leeg als het gram zijn eigen wortel is.
    pub group: Vec<&'a Gram>,
    /// Waarom het gram geen wortel kreeg, als dat zo is.
    pub wortelfout: Option<String>,
}

pub struct Kroniek {
    map: PathBuf,
    /// Een schrijver tegelijk, zodat regels niet door elkaar lopen en een
    /// controle ziet wat er werkelijk ligt. Lezers wachten niet op hem: de
    /// schijf (fsync) gebeurt zonder `staat` vast te houden, en alleen
    /// schrijvers veranderen `staat`.
    schrijver: Mutex<()>,
    /// Per kroniek de grammen, en de indexen.
    staat: RwLock<Staat>,
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
    pub fn open(map: &Path, chronicles: &[&str]) -> Result<Self, String> {
        std::fs::create_dir_all(map).map_err(|e| format!("{}: {e}", map.display()))?;
        let k = Self {
            map: map.to_path_buf(),
            schrijver: Mutex::new(()),
            staat: RwLock::new(Staat::default()),
        };
        k.laad(chronicles)?;
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

    fn lees_staat(&self) -> RwLockReadGuard<'_, Staat> {
        self.staat.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn schrijf_staat(&self) -> RwLockWriteGuard<'_, Staat> {
        self.staat.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Zorg dat deze kronieken in het geheugen staan; een kroniek die er nog
    /// niet is, wordt van schijf gelezen. De wortels van de nieuwe grammen
    /// komen uit hun verwijzingen, ook naar een gram in een andere kroniek die
    /// tegelijk geladen wordt. Een gram dat naar een onbekend id verwijst,
    /// wordt zijn eigen wortel, met een melding.
    fn laad(&self, chronicles: &[&str]) -> Result<(), String> {
        let absent: Vec<&str> = {
            let staat = self.lees_staat();
            chronicles
                .iter()
                .copied()
                .filter(|k| !staat.stapels.contains_key(*k))
                .collect()
        };
        if absent.is_empty() {
            return Ok(());
        }
        // Onder het schrijfslot, zodat niemand tegelijk aan het bestand
        // schrijft terwijl het gelezen (en zo nodig afgekapt) wordt.
        let _schrijver = self.schrijfslot();
        let mut nieuw: Vec<(String, u64, Vec<Gram>)> = Vec::new();
        for k in absent {
            if self.lees_staat().stapels.contains_key(k) || nieuw.iter().any(|(n, _, _)| n == k) {
                continue;
            }
            let (lengte, grams) = lees_bestand(&self.bestand(k)?, k)?;
            nieuw.push((k.to_string(), lengte, grams));
        }
        let mut staat = self.schrijf_staat();
        // De wortels, in rondes: een gram kan verwijzen naar een gram dat
        // later in de lijst staat (een andere kroniek).
        let mut bekend: HashMap<String, String> = HashMap::new();
        loop {
            let mut verder = false;
            for (_, _, grams) in &mut nieuw {
                for g in grams.iter_mut().filter(|g| g.root.is_none()) {
                    let w = match staat.wortel_van(g, &bekend) {
                        Root::Own => g.id.clone(),
                        Root::Van(w) => w,
                        Root::Onbekend(_) | Root::Verschillend => continue,
                    };
                    bekend.insert(g.id.clone(), w.clone());
                    g.root = Some(w);
                    verder = true;
                }
            }
            if !verder {
                break;
            }
        }
        for (k, lengte, grams) in nieuw {
            let mut los = 0_usize;
            staat.stapels.entry(k.clone()).or_default().lengte = lengte;
            for mut g in grams {
                if g.root.is_none() {
                    los += 1;
                    g.root = Some(g.id.clone());
                }
                if staat.per_id.contains_key(&g.id) {
                    return Err(format!("kroniek '{k}': id {} staat er twee keer in", g.id));
                }
                staat.voeg_toe(g);
            }
            if los > 0 {
                tracing::warn!(chronicle = %k, grams = los, "grammen die naar een onbekend gram verwijzen (of naar grammen van verschillende wortels): gelezen als hun eigen wortel");
            }
        }
        Ok(())
    }

    /// Lees uit de stapels van `kronieken`. Lezen gaat nooit naar de schijf:
    /// [`Kroniek::open`] laadt de kronieken van de cel, en een lezer (vaak op
    /// een async-draad) wacht zo niet op een bestand. Een kroniek die niet
    /// geopend is, is een fout.
    fn met_staat<T>(&self, chronicles: &[&str], f: impl FnOnce(&Staat) -> T) -> Result<T, String> {
        let staat = self.lees_staat();
        for k in chronicles {
            self.bestand(k)?;
            if !staat.stapels.contains_key(*k) {
                return Err(format!("kroniek '{k}' is niet geopend"));
            }
        }
        Ok(f(&staat))
    }

    /// Voeg een gram toe. Het gram moet valideren tegen `gram.json`.
    pub fn voeg_toe(&self, gram: &Gram) -> Result<(), String> {
        self.voeg_toe_mits(gram, &[], |_, _| Ok::<(), std::convert::Infallible>(()))?
            .map(|_| ())
            .map_err(|never| match never {})
    }

    /// Voeg een gram toe als `controle` dat toelaat. De controle ziet de
    /// grammen waarnaar het gram verwijst en de grammen van zijn wortel in
    /// `kronieken` (uit de indexen, zonder de rest van de kroniek te
    /// kopiëren; zie [`Zicht`]), zoals ze op dat moment vastliggen, onder
    /// hetzelfde slot als het schrijven. Zo kunnen twee gelijktijdige
    /// verzoeken niet allebei door een controle komen die op het andere had
    /// moeten stuiten. Het gram krijgt zijn wortel voor de controle.
    ///
    /// De buitenste fout is een fout van de opslag; de binnenste is de
    /// weigering van de controle, en dan is er niets vastgelegd.
    pub fn voeg_toe_mits<E>(
        &self,
        gram: &Gram,
        chronicles: &[&str],
        check: impl FnOnce(&mut Gram, &Zicht<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Vastgelegd>, E>, String> {
        self.schrijf_mits(
            gram.clone(),
            chronicles,
            None::<(fn() -> _, fn(String) -> E)>,
            check,
        )
    }

    /// Leg een gram vast zoals [`Kroniek::voeg_toe_mits`], en zet eerst,
    /// onder het schrijfslot, het moment van vastleggen uit `klok` (zie
    /// [`Gram::stempel`]) en een nieuw id (een uuid v7 op dat moment). Zo is de volgorde van de regels in het bestand die
    /// van hun `vastgelegd_op`, ook als twee verzoeken tegelijk komen. De
    /// controle ziet het gestempelde gram. Antwoord: het gram zoals het
    /// vastligt. Weigert het stempel het gram (een gebonden `op_moment` na
    /// het vastleggen), dan maakt `weiger` daar de weigering van. De
    /// controle mag het gram aanvullen met wat pas onder het slot vaststaat.
    pub fn leg_vast_mits<E>(
        &self,
        gram: Gram,
        chronicles: &[&str],
        klok: impl FnOnce() -> DateTime<FixedOffset>,
        weiger: impl FnOnce(String) -> E,
        check: impl FnOnce(&mut Gram, &Zicht<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Vastgelegd>, E>, String> {
        self.schrijf_mits(gram, chronicles, Some((klok, weiger)), check)
    }

    fn schrijf_mits<E>(
        &self,
        mut gram: Gram,
        chronicles: &[&str],
        klok: Option<(
            impl FnOnce() -> DateTime<FixedOffset>,
            impl FnOnce(String) -> E,
        )>,
        check: impl FnOnce(&mut Gram, &Zicht<'_>) -> Result<(), E>,
    ) -> Result<Result<Arc<Vastgelegd>, E>, String> {
        let path = self.bestand(&gram.chronicle)?;
        let mut alle = chronicles.to_vec();
        alle.push(gram.chronicle.as_str());
        self.laad(&alle)?;
        let _schrijver = self.schrijfslot();
        // Zolang wij het schrijfslot hebben, verandert `staat` niet: de
        // controle ziet wat er ligt, ook nadat het leesslot weer los is.
        let (zicht, lengte, laatst) = {
            let staat = self.lees_staat();
            let zicht = staat.zicht(chronicles, &gram);
            gram.root = zicht.root.clone();
            let stapel = staat.stapels.get(&gram.chronicle);
            let lengte = stapel.map_or(0, |s| s.lengte);
            let laatst = stapel.and_then(|s| s.grams.last()).cloned();
            (zicht, lengte, laatst)
        };
        if let Some((klok, weiger)) = klok {
            let niet_voor = laatst.map(|v| v.gram.recorded()).transpose()?;
            let nu = klok();
            if let Err(f) = gram.stempel(nu, niet_voor) {
                return Ok(Err(weiger(f)));
            }
            gram.id = crate::gram::nieuw_id(gram.recorded()?);
        }
        if gram.root.is_none() && zicht.wortelfout.is_none() {
            gram.root = Some(gram.id.clone());
        }
        let zicht = zicht.zicht();
        if let Err(w) = check(&mut gram, &zicht) {
            return Ok(Err(w));
        }
        if let Some(f) = zicht.wortelfout {
            return Err(f);
        }
        if self.lees_staat().per_id.contains_key(&gram.id) {
            return Err(format!("er ligt al een gram met id {}", gram.id));
        }
        let regel = als_regel(&gram)?;
        schrijf_regel(&path, lengte, regel.as_bytes())?;
        let mut staat = self.schrijf_staat();
        staat
            .stapels
            .entry(gram.chronicle.clone())
            .or_default()
            .lengte = lengte + regel.len() as u64;
        Ok(Ok(staat.voeg_toe(gram)))
    }

    /// Zet de startstand in de kroniek, als elke kroniek van `kronieken` leeg
    /// is; onwaar als er al iets lag. Elk bestand wordt in een keer geschreven
    /// (een tijdelijk bestand, dan hernoemd), zodat een onderbroken start
    /// geen half bestand achterlaat. Beslaat de startstand meer dan een
    /// kroniek, dan is dat per bestand, niet over de bestanden heen.
    pub fn zet_startstand(&self, chronicles: &[&str], grams: &[Gram]) -> Result<bool, String> {
        let mut per_kroniek: BTreeMap<&str, String> = BTreeMap::new();
        for g in grams {
            per_kroniek
                .entry(g.chronicle.as_str())
                .or_default()
                .push_str(&als_regel(g)?);
        }
        let mut alle: Vec<&str> = chronicles.to_vec();
        alle.extend(per_kroniek.keys());
        self.laad(&alle)?;
        let _schrijver = self.schrijfslot();
        {
            let staat = self.lees_staat();
            if alle
                .iter()
                .any(|k| staat.stapels.get(*k).is_some_and(|s| !s.grams.is_empty()))
            {
                return Ok(false);
            }
        }
        // De wortels van de startstand, uit haar eigen verwijzingen, in de
        // volgorde van de startstand.
        let mut met_wortel: Vec<Gram> = Vec::with_capacity(grams.len());
        let mut bekend: HashMap<String, String> = HashMap::new();
        for g in grams {
            let mut g = g.clone();
            let mut w = g.id.clone();
            for id in g.refers_to.values() {
                w = bekend.get(id).cloned().ok_or_else(|| {
                    format!(
                        "initial_state: gram {} verwijst naar {id}, dat er niet (eerder) in staat",
                        g.id
                    )
                })?;
            }
            if bekend.insert(g.id.clone(), w.clone()).is_some() {
                return Err(format!("initial_state: id {} staat er twee keer in", g.id));
            }
            g.root = Some(w);
            met_wortel.push(g);
        }
        // Eerst alles naast de kroniek, dan hernoemen: een hernoeming is
        // per bestand atomair.
        let mut klaar = Vec::new();
        for (k, tekst) in &per_kroniek {
            let path = self.bestand(k)?;
            let tijdelijk = path.with_extension("jsonl.nieuw");
            schrijf_bestand(&tijdelijk, tekst.as_bytes())?;
            klaar.push((tijdelijk, path));
        }
        let mut staat = self.schrijf_staat();
        for ((tijdelijk, path), (k, tekst)) in klaar.iter().zip(&per_kroniek) {
            std::fs::rename(tijdelijk, path).map_err(|e| format!("{}: {e}", path.display()))?;
            // Wat hernoemd is, staat ook in het geheugen, ook als een
            // volgende hernoeming mislukt.
            staat.stapels.entry((*k).to_string()).or_default().lengte = tekst.len() as u64;
            for g in met_wortel.iter().filter(|g| g.chronicle == *k) {
                staat.voeg_toe(g.clone());
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
    pub fn alle(&self, chronicles: &[&str]) -> Result<Vec<Arc<Vastgelegd>>, String> {
        self.met_staat(chronicles, |staat| {
            chronicles
                .iter()
                .filter_map(|k| staat.stapels.get(*k))
                .flat_map(|s| s.grams.iter().cloned())
                .collect()
        })
    }

    /// De grammen met deze wortel, over de gegeven kronieken, in de volgorde
    /// van vastleggen.
    pub fn lees_wortel(
        &self,
        chronicles: &[&str],
        root: &str,
    ) -> Result<Vec<Arc<Vastgelegd>>, String> {
        self.met_staat(chronicles, |staat| staat.van_de_wortel(chronicles, root))
    }

    /// Wat een controle over `gram` zou zien als het nu werd vastgelegd (zie
    /// [`Zicht`]), zonder slot: voor een proef, die niets vastlegt. Het gram
    /// krijgt zijn wortel (zijn eigen id als het nergens naar verwijst).
    pub fn zicht_voor(&self, chronicles: &[&str], gram: &mut Gram) -> Result<ZichtEigen, String> {
        let z = self.met_staat(chronicles, |staat| staat.zicht(chronicles, gram))?;
        gram.root = z
            .root
            .clone()
            .or_else(|| z.wortelfout.is_none().then(|| gram.id.clone()));
        Ok(z)
    }

    /// Het gram met dit id, als het in een van deze kronieken ligt.
    pub fn gram(&self, chronicles: &[&str], id: &str) -> Result<Option<Arc<Vastgelegd>>, String> {
        self.met_staat(chronicles, |staat| {
            staat
                .per_id
                .get(id)
                .filter(|v| chronicles.contains(&v.gram.chronicle.as_str()))
                .cloned()
        })
    }
}

/// Lees een kroniek van schijf: de lengte tot de laatste hele regel en de
/// grammen, nog zonder wortel. Een laatste regel zonder regeleinde is
/// onvolledig geschreven: die wordt afgekapt, met een melding, maar pas als
/// de rest leesbaar is. Een tijdelijk bestand van een onderbroken startstand
/// wordt weggehaald. Een kroniek van voor chronolex v0.2.0 (een gram zonder
/// id of `vastgelegd_op`) wordt niet omgezet maar geweigerd.
fn lees_bestand(path: &Path, chronicle: &str) -> Result<(u64, Vec<Gram>), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let tijdelijk = path.with_extension("jsonl.nieuw");
    if tijdelijk.exists() {
        tracing::warn!(bestand = %tijdelijk.display(), "resten van een onderbroken startstand weggehaald");
        std::fs::remove_file(&tijdelijk).map_err(error)?;
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, Vec::new())),
        Err(e) => return Err(error(e)),
    };
    let heel = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let tekst = std::str::from_utf8(&bytes[..heel])
        .map_err(|e| format!("{}: geen UTF-8: {e}", path.display()))?;
    let mut grams = Vec::new();
    for (i, regel) in tekst.lines().enumerate() {
        if regel.trim().is_empty() {
            continue;
        }
        let doc: serde_json::Value = serde_json::from_str(regel)
            .map_err(|e| format!("{} regel {}: {e}", path.display(), i + 1))?;
        if ["id", "recorded_at"].iter().any(|k| doc.get(k).is_none()) {
            return Err(format!(
                "{} regel {}: een gram van voor chronolex v0.2.0 (zonder id of vastgelegd_op); oude kronieken worden niet omgezet: begin met een lege DATA_DIR voor kroniek '{chronicle}'",
                path.display(),
                i + 1
            ));
        }
        let gram: Gram = serde_json::from_value(doc)
            .map_err(|e| format!("{} regel {}: {e}", path.display(), i + 1))?;
        grams.push(gram);
    }
    if heel < bytes.len() {
        tracing::warn!(
            chronicle = %path.display(),
            bytes = bytes.len() - heel,
            "onvolledige laatste regel afgekapt: de runtime stopte tijdens het schrijven, en dat gram is nooit bevestigd"
        );
        let f = OpenOptions::new().write(true).open(path).map_err(error)?;
        f.set_len(heel as u64)
            .and_then(|()| f.sync_all())
            .map_err(error)?;
    }
    Ok((heel as u64, grams))
}

/// Schrijf een regel achter de eerste `lengte` bytes van een kroniek, en
/// wacht tot hij op schijf staat. Wat daarna nog in het bestand stond (de
/// rest van een eerder mislukte schrijfactie) wordt eerst weggehaald, zodat
/// een regel nooit achter een halve regel komt.
fn schrijf_regel(path: &Path, lengte: u64, regel: &[u8]) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(error)?;
    f.set_len(lengte).map_err(error)?;
    f.seek(SeekFrom::Start(lengte)).map_err(error)?;
    f.write_all(regel)
        .and_then(|()| f.sync_data())
        .map_err(error)
}

/// Schrijf een heel bestand en wacht tot het op schijf staat.
fn schrijf_bestand(path: &Path, content: &[u8]) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = std::fs::File::create(path).map_err(error)?;
    f.write_all(content)
        .and_then(|()| f.sync_all())
        .map_err(error)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::testvolger;

    /// Een gram met een nieuw id dat naar het gram met id `wortel` verwijst,
    /// of, als `wortel` nog niet in de kroniek ligt, dat gram zelf.
    fn gram(root: &str) -> Gram {
        crate::gram::testgram(&uuid::Uuid::now_v7().to_string()).met_verwijzing(root)
    }

    trait MetVerwijzing {
        fn met_verwijzing(self, doel: &str) -> Gram;
    }
    impl MetVerwijzing for Gram {
        fn met_verwijzing(mut self, doel: &str) -> Gram {
            self.refers_to.insert("application".into(), doel.into());
            self.root = None;
            self
        }
    }

    /// Het gram dat de wortel `id` is.
    fn root(id: &str) -> Gram {
        crate::gram::testgram(id)
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
        k.voeg_toe(&root(Z1)).unwrap();
        k.voeg_toe(&root(Z2)).unwrap();
        let volger = k
            .voeg_toe_mits(&gram(Z1), K, |_, _| Ok::<(), ()>(()))
            .unwrap()
            .unwrap();
        assert_eq!(volger.gram.root.as_deref(), Some(Z1));
        // Een gram dat naar de volger verwijst, hoort bij dezelfde wortel.
        let verder = testvolger("decision", &volger.gram);
        k.voeg_toe(&verder).unwrap();
        assert_eq!(aantal(&k), 4);
        assert_eq!(k.lees_wortel(K, Z1).unwrap().len(), 3);
        assert_eq!(k.gram(K, &verder.id).unwrap().unwrap().gram.id, verder.id);
        // Na opnieuw openen staat hetzelfde er, uit het bestand, met de wortels.
        drop(k);
        let k = open(dir.path());
        assert_eq!(aantal(&k), 4);
        assert_eq!(k.lees_wortel(K, Z1).unwrap().len(), 3);
        assert_eq!(k.lees_wortel(K, Z2).unwrap().len(), 1);
    }

    /// Een kroniek van voor v0.2.0 (zonder id, met zaakkenmerk) wordt niet
    /// omgezet: de cel weigert haar, met wat te doen.
    #[test]
    fn een_oude_kroniek_wordt_geweigerd() {
        let dir = tempfile::tempdir().unwrap();
        let mut oud = serde_json::to_value(root(Z1)).unwrap();
        let o = oud.as_object_mut().unwrap();
        o.remove("id");
        o.insert("case".into(), "opent".into());
        o.insert("zaakkenmerk".into(), Z1.into());
        std::fs::write(dir.path().join("test_kroniek.jsonl"), format!("{oud}\n")).unwrap();
        let f = Kroniek::open(dir.path(), K).err().unwrap();
        assert!(f.contains("van voor chronolex v0.2.0"), "{f}");
        assert!(f.contains("lege DATA_DIR"), "{f}");
    }

    #[test]
    fn alleen_toevoegen_eerdere_regels_blijven_staan() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&root(Z1)).unwrap();
        let path = dir.path().join("test_kroniek.jsonl");
        let voor = std::fs::read_to_string(&path).unwrap();
        k.voeg_toe(&root(Z2)).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(&voor));
        assert_eq!(after.lines().count(), 2);
    }

    #[test]
    fn ongeldig_gram_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.id = "geen-uuid".into();
        assert!(k.voeg_toe(&g).unwrap_err().contains("id"));
        // Een verwijzing die geen uuid is, en een id dat er al ligt.
        let mut g = root(Z1);
        g.refers_to.insert("application".into(), "geen-uuid".into());
        assert!(k.voeg_toe(&g).is_err());
        k.voeg_toe(&root(Z1)).unwrap();
        assert!(k
            .voeg_toe(&root(Z1))
            .unwrap_err()
            .contains("al een gram met id"));
        assert_eq!(aantal(&k), 1);
    }

    #[test]
    fn een_gram_met_een_ongeldig_op_moment_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.effective_at = "2025-03-12 10:14".into();
        let f = k.voeg_toe(&g).unwrap_err();
        assert!(
            f.contains("ongeldig effective_at '2025-03-12 10:14'"),
            "{f}"
        );
        assert_eq!(aantal(&k), 0);
    }

    #[test]
    fn een_gram_zonder_vastgelegd_op_wordt_niet_vastgelegd() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let mut g = root(Z1);
        g.recorded_at = String::new();
        let f = k.voeg_toe(&g).unwrap_err();
        assert!(f.contains("recorded_at"), "{f}");
        assert_eq!(aantal(&k), 0);
    }

    #[test]
    fn een_weigering_van_de_controle_legt_niets_vast() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&root(Z1)).unwrap();
        let output = k
            .voeg_toe_mits(&gram(Z1), K, |_, zicht| {
                if zicht.group.is_empty() {
                    Ok(())
                } else {
                    Err("er ligt al iets")
                }
            })
            .unwrap();
        assert_eq!(output.err(), Some("er ligt al iets"));
        assert_eq!(aantal(&k), 1);
    }

    /// De controle ziet de grammen waarnaar het gram verwijst en de groep van
    /// zijn wortel, uit de indexen; een gram zonder verwijzing ziet geen
    /// groep, en een gram met een onbekende verwijzing krijgt geen wortel.
    #[test]
    fn de_controle_ziet_doelen_en_groep() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&root(Z1)).unwrap();
        k.voeg_toe(&root(Z2)).unwrap();
        k.voeg_toe(&gram(Z1)).unwrap();
        let mut gezien = (0, 0);
        k.voeg_toe_mits(&gram(Z1), K, |g, zicht| {
            gezien = (zicht.doelen.len(), zicht.group.len());
            assert_eq!(g.root.as_deref(), Some(Z1));
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(gezien, (1, 2));
        let mut aantal_gezien = usize::MAX;
        k.voeg_toe_mits(&root(&uuid::Uuid::now_v7().to_string()), K, |_, zicht| {
            aantal_gezien = zicht.group.len();
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(aantal_gezien, 0);
        let los = gram("00000000-0000-4000-8000-00000000000f");
        let error = k.voeg_toe_mits(&los, K, |_, zicht| match &zicht.wortelfout {
            Some(f) => Err(f.clone()),
            None => Ok(()),
        });
        assert!(error.unwrap().unwrap_err().contains("geen gram"));
    }

    #[test]
    fn gelijktijdige_controles_laten_er_een_door() {
        let dir = tempfile::tempdir().unwrap();
        let k = Arc::new(open(dir.path()));
        k.voeg_toe(&root(Z1)).unwrap();
        let draden: Vec<_> = (0..8)
            .map(|_| {
                let k = k.clone();
                std::thread::spawn(move || {
                    k.voeg_toe_mits(&gram(Z1), K, |_, zicht| {
                        if zicht.group.len() == 1 {
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
        assert_eq!(aantal(&k), 2);
        let path = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 2);
    }

    /// Het stempel `vastgelegd_op` komt onder het schrijfslot: bij
    /// gelijktijdige verzoeken is de volgorde van de regels in het bestand die
    /// van hun `vastgelegd_op`. De klok telt hier op per aanroep, maar de
    /// draden komen in willekeurige volgorde bij het slot; zonder stempel
    /// onder het slot liepen bestand en tijd dan uiteen.
    #[test]
    fn het_stempel_komt_onder_het_slot() {
        use std::sync::atomic::{AtomicI64, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let k = Arc::new(open(dir.path()));
        let teller = Arc::new(AtomicI64::new(0));
        let draden: Vec<_> = (0..16)
            .map(|i| {
                let (k, teller) = (k.clone(), teller.clone());
                std::thread::spawn(move || {
                    let mut g = root(&format!("00000000-0000-4000-8000-{i:012}"));
                    g.recorded_at = "2000-01-01T00:00:00+01:00".into();
                    let klok = || {
                        let s = teller.fetch_add(1, Ordering::SeqCst);
                        DateTime::parse_from_rfc3339("2025-03-12T10:00:00+01:00").unwrap()
                            + chrono::Duration::seconds(s)
                    };
                    k.leg_vast_mits(g, &[], klok, |_| (), |_, _| Ok::<(), ()>(()))
                        .unwrap()
                        .unwrap()
                })
            })
            .collect();
        for d in draden {
            d.join().unwrap();
        }
        let tekst = std::fs::read_to_string(dir.path().join("test_kroniek.jsonl")).unwrap();
        let grams: Vec<Gram> = tekst
            .lines()
            .map(|r| serde_json::from_str::<Gram>(r).unwrap())
            .collect();
        let momenten: Vec<String> = grams.iter().map(|g| g.recorded_at.clone()).collect();
        // Elk gram kreeg onder het slot een eigen id.
        let mut ids: Vec<&str> = grams.iter().map(|g| g.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 16);
        assert_eq!(momenten.len(), 16);
        let mut gesorteerd = momenten.clone();
        gesorteerd.sort();
        assert_eq!(
            momenten, gesorteerd,
            "de bestandsvolgorde is die van vastgelegd_op"
        );
        // Zonder gebonden op_moment schuift het op_moment mee.
        let g: Gram = serde_json::from_str(tekst.lines().next().unwrap()).unwrap();
        assert_eq!(g.effective_at, g.recorded_at);
    }

    /// Loopt de klok terug, dan krijgt een gram niet een eerder
    /// `vastgelegd_op` dan de regel ervoor. Een gebonden `op_moment` na dat
    /// stempel wordt geweigerd.
    #[test]
    fn het_stempel_loopt_niet_terug() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let t = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        let eerst = k
            .leg_vast_mits(
                root(Z1),
                &[],
                || t("2025-03-12T10:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap();
        let daarna = k
            .leg_vast_mits(
                root(Z1),
                &[],
                || t("2025-03-12T09:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(daarna.gram.recorded_at, eerst.gram.recorded_at);
        let mut gebonden = root(Z1);
        gebonden.effective_at = "2025-03-13T00:00:00+01:00".into();
        gebonden.effective_at_legal_basis = Some(vec!["testregeling_aanvraag#1".into()]);
        let f = k
            .leg_vast_mits(
                gebonden,
                &[],
                || t("2025-03-12T11:00:00+01:00"),
                |f| f,
                |_, _| Ok::<(), String>(()),
            )
            .unwrap()
            .unwrap_err();
        assert!(f.contains("na het vastleggen"), "{f}");
        assert_eq!(aantal(&k), 2);
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
        k.voeg_toe(&root(Z1)).unwrap();
        drop(k);
        // De runtime stopte midden in het schrijven van het tweede gram.
        let path = dir.path().join("test_kroniek.jsonl");
        let heel = std::fs::read_to_string(&path).unwrap();
        let tweede = serde_json::to_string(&root(Z2)).unwrap();
        std::fs::write(&path, format!("{heel}{}", &tweede[..40])).unwrap();

        let k = open(dir.path());
        assert_eq!(aantal(&k), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), heel);
        // En daarna gaat het vastleggen gewoon verder, op een hele regel.
        k.voeg_toe(&root(Z2)).unwrap();
        drop(k);
        let k = open(dir.path());
        assert_eq!(aantal(&k), 2);
    }

    #[test]
    fn een_kapotte_regel_middenin_opent_niet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_kroniek.jsonl");
        let g = serde_json::to_string(&root(Z1)).unwrap();
        std::fs::write(&path, format!("{g}\n{{\"kind\": \n{g}\n")).unwrap();
        let error = Kroniek::open(dir.path(), K).err().unwrap();
        assert!(error.contains("test_kroniek.jsonl regel 2"), "{error}");
        // Het bestand is niet aangeraakt.
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 3);
        // Ook niet als er daarbij een halve laatste regel staat: eerst lezen,
        // dan pas afkappen.
        let met_staart = format!("{g}\n{{\"kind\": \n{g}\n{{\"ki");
        std::fs::write(&path, &met_staart).unwrap();
        assert!(Kroniek::open(dir.path(), K).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), met_staart);
    }

    #[test]
    fn een_rest_van_een_mislukte_schrijfactie_wordt_overschreven() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&root(Z1)).unwrap();
        // Een eerdere schrijfactie liet een halve regel achter die niet
        // teruggezet kon worden; de kroniek weet nog de goede lengte.
        let path = dir.path().join("test_kroniek.jsonl");
        let heel = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{heel}{{\"half")).unwrap();
        k.voeg_toe(&root(Z2)).unwrap();
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
        assert!(k.zet_startstand(K, &[root(Z1)]).unwrap());
        assert_eq!(aantal(&k), 1);
    }

    #[test]
    fn de_startstand_in_een_keer_en_alleen_in_een_lege_kroniek() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        let stand = [root(Z1), root(Z2)];
        assert!(k.zet_startstand(K, &stand).unwrap());
        assert_eq!(aantal(&k), 2);
        let path = dir.path().join("test_kroniek.jsonl");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
        assert!(!dir.path().join("test_kroniek.jsonl.nieuw").exists());
        // Niet nog eens.
        assert!(!k.zet_startstand(K, &stand).unwrap());
        assert_eq!(aantal(&k), 2);
        // Een ongeldig gram: niets geschreven.
        let leeg = tempfile::tempdir().unwrap();
        let k = open(leeg.path());
        let mut slecht = root(Z2);
        slecht.effective_at = "gisteren".into();
        assert!(k.zet_startstand(K, &[root(Z1), slecht]).is_err());
        assert_eq!(aantal(&k), 0);
        assert!(!leeg.path().join("test_kroniek.jsonl").exists());
    }

    #[test]
    fn de_yaml_wordt_een_keer_gemaakt() {
        let dir = tempfile::tempdir().unwrap();
        let k = open(dir.path());
        k.voeg_toe(&root(Z1)).unwrap();
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
