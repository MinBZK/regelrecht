// De query-laag is als geheel geschreven (lezen, schrijven, verwijderen per
// tabel); de demo raakt er nu een deel van aan. Dat is geen dode code die weg
// moet maar een complete laag met een deels gebruikte bovenkant — de ontbrekende
// helft weghalen zou de volgende die er iets bijbouwt laten raden welke helft er
// ooit was.
#![allow(dead_code)]

//! Partijregister van de Napp: de registratietaak van de Nederlandse
//! autoriteit politieke partijen (KvK-nummer ↔ geregistreerde aanduiding,
//! organisatiemodel, decentrale verkiezingsuitslagen).
//!
//! Het register leeft in SQLite (`register_partijen`, `register_uitslagen`,
//! `register_gebieden`, zie `db::init`) en wordt beheerd via de
//! beoordelaarsomgeving (`beheer.rs`). Deze module is de query-laag.
//!
//! Het bestand staat bewust zonder witruimte op schijf (483 KB in plaats van
//! 652 KB): de pre-commit-hook weigert bestanden boven 500 KB, en dit is
//! gegenereerde data die niemand met de hand leest. Herformatteer het niet.
//!
//! De seed-snapshot in `data/partijregister.json` wordt gegenereerd door
//! `scripts/bouw_register.py` uit drie open databronnen: de
//! verkiezingsuitslag Tweede Kamer 2025 en de gemeenteraadsuitslagen 2026
//! (Kiesraad, data.overheid.nl) en de inwoneraantallen per gemeente
//! (CBS StatLine). De KvK-nummers zijn synthetisch: de koppeling
//! rechtspersoon-aanduiding is geen open data en is precies wat de Napp
//! bij registratie vastlegt. Bij startup worden de tabellen uit de snapshot
//! gevuld wanneer ze leeg zijn; daarna is de database de bron van waarheid.
//! `demo_voorbeelden` blijft statisch uit de JSON komen.
//!
//! Het datamodel kent decentrale organen GEMEENTERAAD, PROVINCIALE_STATEN
//! en WATERSCHAP. Kamerzetels staan als kolom op de partij, niet als
//! uitslag-rij — zie de toelichting bij het schema in `db.rs`.
//!
//! Een onbekend KvK-nummer mag gewoon een aanvraag indienen (AWB 4:1) —
//! de wet wijst dan af. Het register sluit de aanvraagroute niet af.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::OnceLock;

/// Status of a register record: GEVERIFIEERD when the legal entity behind
/// the aanduiding is known and checked; ONGEKOPPELD when the aanduiding
/// comes straight from the election result and the legal entity is still
/// unknown (kvk_nummer is then a synthetic placeholder from the source).
pub const STATUS_GEVERIFIEERD: &str = "GEVERIFIEERD";
pub const STATUS_ONGEKOPPELD: &str = "ONGEKOPPELD";

fn default_status() -> String {
    STATUS_GEVERIFIEERD.to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Partij {
    pub kvk_nummer: String,
    pub naam: String,
    /// CENTRAAL (afdelingen onder een KvK) of DECENTRAAL (afdelingen als
    /// eigen rechtspersoon) — Wpp-organisatiemodellen.
    #[serde(default)]
    pub organisatiemodel: String,
    /// Zetels in Eerste + Tweede Kamer (bron: Kiesraad, TK2025).
    pub kamerzetels: i64,
    #[serde(default)]
    pub moederpartij_kvk: Option<String>,
    /// Rekening van de rechtspersoon voor uitbetaling (eigen opgave door het
    /// tekenbevoegd bestuur, zie `rekening.rs`). Niet in de seed-snapshot.
    #[serde(default)]
    pub iban: Option<String>,
    #[serde(default)]
    pub iban_tenaamstelling: Option<String>,
    /// GEVERIFIEERD | ONGEKOPPELD (zie de constanten hierboven).
    #[serde(default = "default_status")]
    pub status: String,
    pub decentrale_uitslagen: Vec<Uitslag>,
}

impl Partij {
    /// Whether the legal entity behind this record is still unknown.
    pub fn is_ongekoppeld(&self) -> bool {
        self.status == STATUS_ONGEKOPPELD
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Uitslag {
    /// GEMEENTERAAD | PROVINCIALE_STATEN | WATERSCHAP
    pub orgaan: String,
    pub gebied_code: String,
    pub zetels: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gebied {
    pub orgaan: String,
    pub code: String,
    pub naam: String,
    /// Bron: CBS StatLine.
    pub inwoneraantal: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DemoVoorbeeld {
    pub kvk_nummer: String,
    pub naam: String,
    #[serde(default)]
    pub profiel: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Register {
    pub partijen: Vec<Partij>,
    pub gebieden: Vec<Gebied>,
    pub demo_voorbeelden: Vec<DemoVoorbeeld>,
}

static SNAPSHOT: OnceLock<Register> = OnceLock::new();

/// The static JSON snapshot: seed source and home of the demo examples.
fn snapshot() -> &'static Register {
    SNAPSHOT.get_or_init(|| {
        // Het bestand is met include_str! ingebakken: is het ongeldig, dan is
        // dit image stuk en hoort het bij de eerste aanroep om te vallen, niet
        // stil een leeg register op te leveren.
        #[allow(clippy::expect_used)]
        {
            serde_json::from_str(include_str!("../data/partijregister.json"))
                .expect("partijregister.json is geen geldig register")
        }
    })
}

/// Demo examples for the mocked eHerkenning login (static, not in the DB).
pub fn demo_voorbeelden() -> &'static [DemoVoorbeeld] {
    &snapshot().demo_voorbeelden
}

/// Seed the register tables from the JSON snapshot when they are empty.
/// Combined lists (GROENLINKS / PvdA in provinciale staten) appear twice in
/// the source data for the same orgaan/gebied; their zetels are summed into
/// a single row via the upsert.
pub async fn seed_if_empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM register_partijen")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let reg = snapshot();
    let mut tx = pool.begin().await?;
    for g in &reg.gebieden {
        sqlx::query(
            "INSERT INTO register_gebieden (orgaan, code, naam, inwoneraantal) VALUES (?, ?, ?, ?)",
        )
        .bind(&g.orgaan)
        .bind(&g.code)
        .bind(&g.naam)
        .bind(g.inwoneraantal)
        .execute(&mut *tx)
        .await?;
    }
    for p in &reg.partijen {
        sqlx::query(
            "INSERT INTO register_partijen (kvk_nummer, naam, organisatiemodel, kamerzetels, moederpartij_kvk, status)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&p.kvk_nummer)
        .bind(&p.naam)
        .bind(&p.organisatiemodel)
        .bind(p.kamerzetels)
        .bind(&p.moederpartij_kvk)
        .bind(&p.status)
        .execute(&mut *tx)
        .await?;
        for u in &p.decentrale_uitslagen {
            sqlx::query(
                "INSERT INTO register_uitslagen (kvk_nummer, orgaan, gebied_code, zetels)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(kvk_nummer, orgaan, gebied_code)
                 DO UPDATE SET zetels = zetels + excluded.zetels",
            )
            .bind(&p.kvk_nummer)
            .bind(&u.orgaan)
            .bind(&u.gebied_code)
            .bind(u.zetels)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    tracing::info!(
        partijen = reg.partijen.len(),
        gebieden = reg.gebieden.len(),
        "partijregister geseed uit snapshot"
    );
    Ok(())
}

fn row_to_gebied(row: &sqlx::sqlite::SqliteRow) -> Gebied {
    Gebied {
        orgaan: row.get("orgaan"),
        code: row.get("code"),
        naam: row.get("naam"),
        inwoneraantal: row.get("inwoneraantal"),
    }
}

pub async fn partij_by_kvk(pool: &SqlitePool, kvk: &str) -> anyhow::Result<Option<Partij>> {
    let Some(row) = sqlx::query("SELECT * FROM register_partijen WHERE kvk_nummer = ?")
        .bind(kvk)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let uitslagen = sqlx::query(
        "SELECT orgaan, gebied_code, zetels FROM register_uitslagen
         WHERE kvk_nummer = ? ORDER BY orgaan, gebied_code",
    )
    .bind(kvk)
    .fetch_all(pool)
    .await?;
    Ok(Some(Partij {
        kvk_nummer: row.get("kvk_nummer"),
        naam: row.get("naam"),
        organisatiemodel: row.get("organisatiemodel"),
        kamerzetels: row.get("kamerzetels"),
        moederpartij_kvk: row.get("moederpartij_kvk"),
        iban: row.get("iban"),
        iban_tenaamstelling: row.get("iban_tenaamstelling"),
        status: row.get("status"),
        decentrale_uitslagen: uitslagen
            .iter()
            .map(|r| Uitslag {
                orgaan: r.get("orgaan"),
                gebied_code: r.get("gebied_code"),
                zetels: r.get("zetels"),
            })
            .collect(),
    }))
}

/// Lookup by code alone (codes are unique across organen in the dataset);
/// kept as part of the register API even though aanspraken_voor now joins.
#[allow(dead_code)]
pub async fn gebied_by_code(pool: &SqlitePool, code: &str) -> anyhow::Result<Option<Gebied>> {
    let row = sqlx::query("SELECT * FROM register_gebieden WHERE code = ?")
        .bind(code)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_gebied))
}

pub async fn gebied_by_orgaan_code(
    pool: &SqlitePool,
    orgaan: &str,
    code: &str,
) -> anyhow::Result<Option<Gebied>> {
    let row = sqlx::query("SELECT * FROM register_gebieden WHERE orgaan = ? AND code = ?")
        .bind(orgaan)
        .bind(code)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_gebied))
}

#[allow(dead_code)]
pub async fn uitslag_by_kvk_gebied(
    pool: &SqlitePool,
    kvk: &str,
    gebied_code: &str,
) -> anyhow::Result<Option<Uitslag>> {
    let row = sqlx::query(
        "SELECT orgaan, gebied_code, zetels FROM register_uitslagen
         WHERE kvk_nummer = ? AND gebied_code = ?",
    )
    .bind(kvk)
    .bind(gebied_code)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Uitslag {
        orgaan: r.get("orgaan"),
        gebied_code: r.get("gebied_code"),
        zetels: r.get("zetels"),
    }))
}

/// One decentralized election result joined with its gebied (name and
/// inhabitants), as needed to build aanvraag components and the beheer view.
#[derive(Debug, Clone, Serialize)]
pub struct UitslagMetGebied {
    pub orgaan: String,
    pub gebied_code: String,
    pub zetels: i64,
    pub gebied_naam: Option<String>,
    pub inwoneraantal: i64,
}

pub async fn uitslagen_met_gebied(
    pool: &SqlitePool,
    kvk: &str,
) -> anyhow::Result<Vec<UitslagMetGebied>> {
    let rows = sqlx::query(
        "SELECT u.orgaan, u.gebied_code, u.zetels, g.naam AS gebied_naam,
                COALESCE(g.inwoneraantal, 0) AS inwoneraantal
         FROM register_uitslagen u
         LEFT JOIN register_gebieden g ON g.orgaan = u.orgaan AND g.code = u.gebied_code
         WHERE u.kvk_nummer = ?
         ORDER BY u.orgaan, COALESCE(g.naam, u.gebied_code) COLLATE NOCASE",
    )
    .bind(kvk)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| UitslagMetGebied {
            orgaan: r.get("orgaan"),
            gebied_code: r.get("gebied_code"),
            zetels: r.get("zetels"),
            gebied_naam: r.get("gebied_naam"),
            inwoneraantal: r.get("inwoneraantal"),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Beheer-queries (beoordelaarsomgeving)
// ---------------------------------------------------------------------------

/// Summary row for the register management list.
#[derive(Debug, Serialize)]
pub struct PartijOverzicht {
    pub kvk_nummer: String,
    pub naam: String,
    pub organisatiemodel: String,
    pub kamerzetels: i64,
    pub moederpartij_kvk: Option<String>,
    pub status: String,
    pub aantal_uitslagen: i64,
}

/// Search parties by name or KvK number, sorted by name, paginated.
/// Returns the total match count alongside the page.
pub async fn zoek_partijen(
    pool: &SqlitePool,
    zoek: &str,
    offset: i64,
    limit: i64,
) -> anyhow::Result<(i64, Vec<PartijOverzicht>)> {
    let patroon = format!("%{}%", zoek.trim());
    let totaal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM register_partijen WHERE naam LIKE ? OR kvk_nummer LIKE ?",
    )
    .bind(&patroon)
    .bind(&patroon)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query(
        "SELECT p.kvk_nummer, p.naam, p.organisatiemodel, p.kamerzetels, p.moederpartij_kvk,
                p.status,
                (SELECT COUNT(*) FROM register_uitslagen u WHERE u.kvk_nummer = p.kvk_nummer)
                    AS aantal_uitslagen
         FROM register_partijen p
         WHERE p.naam LIKE ? OR p.kvk_nummer LIKE ?
         ORDER BY p.naam COLLATE NOCASE, p.kvk_nummer
         LIMIT ? OFFSET ?",
    )
    .bind(&patroon)
    .bind(&patroon)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok((
        totaal,
        rows.iter()
            .map(|r| PartijOverzicht {
                kvk_nummer: r.get("kvk_nummer"),
                naam: r.get("naam"),
                organisatiemodel: r.get("organisatiemodel"),
                kamerzetels: r.get("kamerzetels"),
                moederpartij_kvk: r.get("moederpartij_kvk"),
                status: r.get("status"),
                aantal_uitslagen: r.get("aantal_uitslagen"),
            })
            .collect(),
    ))
}

/// ONGEKOPPELDE aanduidingen (legal entity unknown) matching the search
/// term, alphabetically, capped at `limit`. Used by the claim flow.
pub async fn zoek_ongekoppelde_partijen(
    pool: &SqlitePool,
    zoek: &str,
    limit: i64,
) -> anyhow::Result<Vec<Partij>> {
    let patroon = format!("%{}%", zoek.trim());
    let rows = sqlx::query(
        "SELECT kvk_nummer FROM register_partijen
         WHERE status = ? AND naam LIKE ?
         ORDER BY naam COLLATE NOCASE, kvk_nummer
         LIMIT ?",
    )
    .bind(STATUS_ONGEKOPPELD)
    .bind(&patroon)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut partijen = Vec::with_capacity(rows.len());
    for row in &rows {
        let kvk: String = row.get("kvk_nummer");
        if let Some(p) = partij_by_kvk(pool, &kvk).await? {
            partijen.push(p);
        }
    }
    Ok(partijen)
}

pub async fn insert_partij(
    pool: &SqlitePool,
    kvk: &str,
    naam: &str,
    organisatiemodel: &str,
    moederpartij_kvk: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO register_partijen (kvk_nummer, naam, organisatiemodel, kamerzetels, moederpartij_kvk)
         VALUES (?, ?, ?, 0, ?)",
    )
    .bind(kvk)
    .bind(naam)
    .bind(organisatiemodel)
    .bind(moederpartij_kvk)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns false when the party does not exist.
pub async fn update_partij(
    pool: &SqlitePool,
    kvk: &str,
    naam: &str,
    organisatiemodel: &str,
    moederpartij_kvk: Option<&str>,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE register_partijen SET naam = ?, organisatiemodel = ?, moederpartij_kvk = ?
         WHERE kvk_nummer = ?",
    )
    .bind(naam)
    .bind(organisatiemodel)
    .bind(moederpartij_kvk)
    .bind(kvk)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Store the bank account of a registered legal entity. Returns false when
/// the party is not in the register (the claim flow, on a parallel branch,
/// is the route that creates the registration first).
pub async fn update_rekening(
    pool: &SqlitePool,
    kvk: &str,
    iban: &str,
    tenaamstelling: &str,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE register_partijen SET iban = ?, iban_tenaamstelling = ? WHERE kvk_nummer = ?",
    )
    .bind(iban)
    .bind(tenaamstelling)
    .bind(kvk)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn uitslag_exists(
    pool: &SqlitePool,
    kvk: &str,
    orgaan: &str,
    gebied_code: &str,
) -> anyhow::Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM register_uitslagen
         WHERE kvk_nummer = ? AND orgaan = ? AND gebied_code = ?",
    )
    .bind(kvk)
    .bind(orgaan)
    .bind(gebied_code)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn insert_uitslag(
    pool: &SqlitePool,
    kvk: &str,
    orgaan: &str,
    gebied_code: &str,
    zetels: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO register_uitslagen (kvk_nummer, orgaan, gebied_code, zetels)
         VALUES (?, ?, ?, ?)",
    )
    .bind(kvk)
    .bind(orgaan)
    .bind(gebied_code)
    .bind(zetels)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns false when no matching uitslag row existed.
pub async fn delete_uitslag(
    pool: &SqlitePool,
    kvk: &str,
    orgaan: &str,
    gebied_code: &str,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "DELETE FROM register_uitslagen
         WHERE kvk_nummer = ? AND orgaan = ? AND gebied_code = ?",
    )
    .bind(kvk)
    .bind(orgaan)
    .bind(gebied_code)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Gebieden for the beheer UI (uitslag-toevoegen form), optionally filtered
/// by orgaan, sorted by name.
pub async fn list_gebieden(pool: &SqlitePool, orgaan: Option<&str>) -> anyhow::Result<Vec<Gebied>> {
    let rows = match orgaan {
        Some(orgaan) => {
            sqlx::query(
                "SELECT * FROM register_gebieden WHERE orgaan = ? ORDER BY naam COLLATE NOCASE",
            )
            .bind(orgaan)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query("SELECT * FROM register_gebieden ORDER BY orgaan, naam COLLATE NOCASE")
                .fetch_all(pool)
                .await?
        }
    };
    Ok(rows.iter().map(row_to_gebied).collect())
}

/// Het eerste vrije nummer in de demo-reeks vanaf `vanaf`: nog niet in het
/// register (een bevestigde claim hangt het nummer aan een partij) en
/// zonder lopende claim. Geeft None als de reeks op is.
pub async fn vrij_demo_kvk(pool: &SqlitePool, vanaf: &str) -> anyhow::Result<Option<String>> {
    let start: i64 = vanaf.parse().unwrap_or(90_000_001);
    for kandidaat in start..start + 99 {
        let kvk = kandidaat.to_string();
        let in_register: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM register_partijen WHERE kvk_nummer = ?")
                .bind(&kvk)
                .fetch_one(pool)
                .await?;
        let open_claim: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM register_claims WHERE kvk_nummer = ? AND status = 'OPEN'",
        )
        .bind(&kvk)
        .fetch_one(pool)
        .await?;
        if in_register == 0 && open_claim == 0 {
            return Ok(Some(kvk));
        }
    }
    Ok(None)
}
