//! De kroniek: append-only opslag van grammen, een JSON-regel per gram in
//! `DATA_DIR/<cel>/<chronicle>.jsonl`.
//!
//! Er is bewust geen pad om een gram te wijzigen of te verwijderen. Een
//! correctie of herstel is een nieuw gram.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::stroom::Gram;

pub struct Kroniek {
    map: PathBuf,
    /// Een schrijver tegelijk, zodat regels niet door elkaar lopen; een
    /// lezer wacht op hem, zodat hij geen half geschreven regel ziet.
    schrijver: Mutex<()>,
}

impl Kroniek {
    /// Open (en maak zo nodig) de map met kronieken.
    pub fn open(map: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(map).map_err(|e| format!("{}: {e}", map.display()))?;
        Ok(Self {
            map: map.to_path_buf(),
            schrijver: Mutex::new(()),
        })
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

    /// Voeg een gram toe. Het gram moet valideren tegen `gram.json`.
    pub fn voeg_toe(&self, gram: &Gram) -> Result<(), String> {
        gram.valideer()
            .map_err(|f| format!("gram valideert niet: {}", f.join("; ")))?;
        let pad = self.bestand(&gram.chronicle)?;
        let mut regel = serde_json::to_string(gram).map_err(|e| e.to_string())?;
        regel.push('\n');
        let _slot = self
            .schrijver
            .lock()
            .map_err(|_| "kroniek vergrendeld".to_string())?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&pad)
            .map_err(|e| format!("{}: {e}", pad.display()))?;
        f.write_all(regel.as_bytes())
            .and_then(|()| f.sync_data())
            .map_err(|e| format!("{}: {e}", pad.display()))
    }

    /// Alle grammen van een kroniek, in de volgorde van vastleggen.
    pub fn lees(&self, chronicle: &str) -> Result<Vec<Gram>, String> {
        let pad = self.bestand(chronicle)?;
        let _slot = self
            .schrijver
            .lock()
            .map_err(|_| "kroniek vergrendeld".to_string())?;
        let f = match std::fs::File::open(&pad) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}: {e}", pad.display())),
        };
        let mut grammen = Vec::new();
        for (i, regel) in BufReader::new(f).lines().enumerate() {
            let regel = regel.map_err(|e| format!("{}: {e}", pad.display()))?;
            if regel.trim().is_empty() {
                continue;
            }
            let gram: Gram = serde_json::from_str(&regel)
                .map_err(|e| format!("{} regel {}: {e}", pad.display(), i + 1))?;
            grammen.push(gram);
        }
        Ok(grammen)
    }

    /// De grammen van een zaak.
    pub fn lees_zaak(&self, chronicle: &str, zaakkenmerk: &str) -> Result<Vec<Gram>, String> {
        Ok(self
            .lees(chronicle)?
            .into_iter()
            .filter(|g| g.zaakkenmerk == zaakkenmerk)
            .collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::stroom::StroomVerwijzing;
    use serde_json::json;

    fn gram(zaak: &str) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            type_: "indiening".into(),
            soort: Some("melding".into()),
            name: "melding_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            grondslag: vec!["testregeling_aanvraag#1".into()],
            op_moment: "2025-03-12T10:14:03+01:00".into(),
            zaakkenmerk: zaak.into(),
            stroom: StroomVerwijzing {
                id: "test".into(),
                sha256: "a".repeat(64),
            },
            herkomst: None,
            fields: json!({"x": 1}).as_object().unwrap().clone(),
        }
    }

    const Z1: &str = "00000000-0000-4000-8000-000000000001";
    const Z2: &str = "00000000-0000-4000-8000-000000000002";

    #[test]
    fn toevoegen_en_lezen() {
        let dir = tempfile::tempdir().unwrap();
        let k = Kroniek::open(dir.path()).unwrap();
        assert!(k.lees("test_kroniek").unwrap().is_empty());
        k.voeg_toe(&gram(Z1)).unwrap();
        k.voeg_toe(&gram(Z2)).unwrap();
        k.voeg_toe(&gram(Z1)).unwrap();
        assert_eq!(k.lees("test_kroniek").unwrap().len(), 3);
        assert_eq!(k.lees_zaak("test_kroniek", Z1).unwrap().len(), 2);
    }

    #[test]
    fn alleen_toevoegen_eerdere_regels_blijven_staan() {
        let dir = tempfile::tempdir().unwrap();
        let k = Kroniek::open(dir.path()).unwrap();
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
        let k = Kroniek::open(dir.path()).unwrap();
        let mut g = gram(Z1);
        g.zaakkenmerk = "geen-uuid".into();
        assert!(k.voeg_toe(&g).unwrap_err().contains("zaakkenmerk"));
        assert!(k.lees("test_kroniek").unwrap().is_empty());
    }

    #[test]
    fn kroniek_naam_is_geen_pad() {
        let dir = tempfile::tempdir().unwrap();
        let k = Kroniek::open(dir.path()).unwrap();
        assert!(k.lees("../elders").is_err());
    }
}
