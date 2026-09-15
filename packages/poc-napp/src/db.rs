//! SQLite persistence: aanvragen, besluiten, betaalopdrachten.
//!
//! Het aanvraagmodel volgt de rechtspersoon (Wpp art. 27): één samengestelde
//! jaaraanvraag per partij per subsidiejaar, met componenten (landelijk en
//! per decentraal orgaan/gebied). Het besluit is één beschikking met een
//! specificatie per component en één totaalbedrag. De engine blijft
//! stateless (RFC-008); deze laag persisteert de besluit-state.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

pub async fn init(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS aanvragen (
            id TEXT PRIMARY KEY,
            kvk_nummer TEXT NOT NULL,
            partij_naam TEXT NOT NULL,
            subsidiejaar INTEGER NOT NULL,
            componenten TEXT NOT NULL,
            parameters TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'BEHANDELING',
            aanvraag_datum TEXT NOT NULL,
            beslistermijn_einddatum TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS besluiten (
            id TEXT PRIMARY KEY,
            aanvraag_id TEXT NOT NULL UNIQUE REFERENCES aanvragen(id),
            subsidie_toegekend INTEGER NOT NULL,
            subsidiebedrag INTEGER NOT NULL,
            componenten TEXT NOT NULL,
            motivering TEXT NOT NULL,
            besluit_datum TEXT NOT NULL,
            bekendmaking_datum TEXT,
            bezwaartermijn_startdatum TEXT,
            bezwaartermijn_einddatum TEXT,
            beoordelaar TEXT NOT NULL,
            -- Het bewijs van het besluit: peildatum, wetversies en per
            -- component de parameters en outputs van de evaluatie (JSON).
            bewijs TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        -- Status: AANGEMAAKT (klaar voor het betaalsysteem, met rekening)
        -- of AANGEHOUDEN (geen rekening bekend bij verlening; wordt demo-
        -- materiaal voor de claim-flow).
        CREATE TABLE IF NOT EXISTS betaalopdrachten (
            id TEXT PRIMARY KEY,
            besluit_id TEXT NOT NULL REFERENCES besluiten(id),
            partij_naam TEXT NOT NULL,
            bedrag INTEGER NOT NULL,
            iban TEXT NULL,
            tenaamstelling TEXT NULL,
            status TEXT NOT NULL DEFAULT 'AANGEMAAKT',
            -- Uiterste betaaldatum (AWB 4:87: zes weken na bekendmaking),
            -- gezet bij de bekendmaking van het besluit.
            betaaltermijn_einddatum TEXT NULL,
            uitgevoerd_at TEXT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        -- Partijregister (registratietaak van de Napp), beheerd in de
        -- beoordelaarsomgeving en geseed uit data/partijregister.json.
        -- Schemakeuze: kamerzetels staan als kolom op register_partijen, niet
        -- als uitslag-rij met orgaan TWEEDE_KAMER. Uitslag-rijen verwijzen
        -- naar een gebied in register_gebieden (met inwoneraantal); de Kamer
        -- heeft geen gebied, dus een uitslag-rij zou een kunstmatig gebied of
        -- NULL-joins vereisen. De kolom volgt bovendien het Partij-domeinmodel.
        -- Eén rekening per rechtspersoon (Wpp art. 27: de subsidie wordt
        -- verstrekt aan de rechtspersoon; afdelingen van een centraal
        -- georganiseerde partij hebben geen rechtspersoonlijkheid). Eigen
        -- opgave door het tekenbevoegd bestuur, met IBAN-naam-controle.
        -- status: GEVERIFIEERD (rechtspersoon bekend en getoetst) of
        -- ONGEKOPPELD (aanduiding uit de uitslag waarvan de rechtspersoon
        -- nog onbekend is; kvk_nummer is dan een placeholder uit de bron).
        CREATE TABLE IF NOT EXISTS register_partijen (
            kvk_nummer TEXT PRIMARY KEY,
            naam TEXT NOT NULL,
            organisatiemodel TEXT NOT NULL DEFAULT 'CENTRAAL',
            kamerzetels INTEGER NOT NULL DEFAULT 0,
            moederpartij_kvk TEXT REFERENCES register_partijen(kvk_nummer),
            iban TEXT NULL,
            iban_tenaamstelling TEXT NULL,
            status TEXT NOT NULL DEFAULT 'GEVERIFIEERD'
        );
        CREATE TABLE IF NOT EXISTS register_uitslagen (
            kvk_nummer TEXT NOT NULL REFERENCES register_partijen(kvk_nummer),
            orgaan TEXT NOT NULL,
            gebied_code TEXT NOT NULL,
            zetels INTEGER NOT NULL,
            PRIMARY KEY (kvk_nummer, orgaan, gebied_code)
        );
        CREATE TABLE IF NOT EXISTS register_gebieden (
            orgaan TEXT NOT NULL,
            code TEXT NOT NULL,
            naam TEXT NOT NULL,
            inwoneraantal INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (orgaan, code)
        );
        -- Claims: een rechtspersoon (kvk_nummer, via eHerkenning) claimt een
        -- ONGEKOPPELDE aanduiding (doel_kvk = placeholder-nummer van het
        -- geclaimde record). hr_toets bevat het opgeslagen resultaat van de
        -- (gemockte) Handelsregister-raadpleging als JSON.
        CREATE TABLE IF NOT EXISTS register_claims (
            id TEXT PRIMARY KEY,
            kvk_nummer TEXT NOT NULL,
            doel_kvk TEXT NOT NULL,
            aanduiding TEXT NOT NULL,
            hr_toets TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'OPEN',
            reden_afwijzing TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            beoordeeld_door TEXT,
            beoordeeld_at TEXT
        );
        -- Bezwaren tegen beschikkingen (AWB hoofdstuk 6/7). De status volgt
        -- de bezwaarprocedure uit de AWB-YAML; het AWB-oordeel over het
        -- bezwaarschrift (6:5/6:6/6:9) staat als JSON in toets.
        CREATE TABLE IF NOT EXISTS bezwaren (
            id TEXT PRIMARY KEY,
            besluit_id TEXT NOT NULL REFERENCES besluiten(id),
            aanvraag_id TEXT NOT NULL REFERENCES aanvragen(id),
            kvk_nummer TEXT NOT NULL,
            naam_indiener TEXT NOT NULL,
            adres_indiener TEXT,
            gronden TEXT,
            ondertekend INTEGER NOT NULL DEFAULT 0,
            ontvangen_op TEXT NOT NULL,
            toets TEXT,
            status TEXT NOT NULL,
            beslistermijn_einddatum TEXT,
            gehoord INTEGER,
            afzien_grond TEXT,
            beslissing TEXT,
            beslissing_motivering TEXT,
            beslissing_datum TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(pool)
    .await?;
    // Migraties voor bestaande databases; falen (genegeerd) wanneer de
    // kolom al bestaat.
    for migratie in [
        "ALTER TABLE besluiten ADD COLUMN bewijs TEXT",
        "ALTER TABLE betaalopdrachten ADD COLUMN betaaltermijn_einddatum TEXT",
        "ALTER TABLE betaalopdrachten ADD COLUMN uitgevoerd_at TEXT",
    ] {
        let _ = sqlx::query(migratie).execute(pool).await;
    }
    Ok(())
}

/// Eén aanspraak binnen een aanvraag: de landelijke component of een
/// decentraal orgaan/gebied. De gegevens komen uit het partijregister en
/// worden bij indiening bevroren (zoals een beschikking hoort te doen).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// "LANDELIJK" of "{orgaan}:{gebied_code}"
    pub key: String,
    pub soort: String, // LANDELIJK | DECENTRAAL
    #[serde(default)]
    pub orgaan: Option<String>,
    #[serde(default)]
    pub gebied_code: Option<String>,
    #[serde(default)]
    pub gebied: Option<String>,
    /// Kamerzetels (landelijk) of zetels in het orgaan (decentraal).
    pub zetels: i64,
    #[serde(default)]
    pub inwoneraantal: i64,
}

/// Uitsplitsing van de landelijke component in de vier delen van art. 14
/// Wpp: de partij zelf (a) en de geoormerkte bedragen voor de aangewezen
/// neveninstellingen (b, c, d).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandelijkeDelen {
    pub partij: i64,
    pub wetenschappelijk_instituut: i64,
    pub jongerenorganisatie: i64,
    pub buitenland: i64,
}

/// Uitkomst per component, vastgelegd in het besluit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentUitkomst {
    #[serde(flatten)]
    pub component: Component,
    pub toegekend: bool,
    pub bedrag: i64,
    /// Alleen voor de landelijke component: de delen van art. 14.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delen: Option<LandelijkeDelen>,
    /// Bij afwijzing: de motiveringszin, opgebouwd uit de per-voorwaarde
    /// outputs van de wet (art. 6/7), niet uit drempels in de uitvoering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afwijzingsgrond: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Aanvraag {
    pub id: String,
    pub kvk_nummer: String,
    pub partij_naam: String,
    pub subsidiejaar: i64,
    pub componenten: Vec<Component>,
    pub parameters: serde_json::Value,
    pub status: String,
    pub aanvraag_datum: String,
    pub beslistermijn_einddatum: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct Besluit {
    pub id: String,
    pub aanvraag_id: String,
    pub subsidie_toegekend: bool,
    pub subsidiebedrag: i64,
    pub componenten: Vec<ComponentUitkomst>,
    pub motivering: String,
    pub besluit_datum: String,
    pub bekendmaking_datum: Option<String>,
    pub bezwaartermijn_startdatum: Option<String>,
    pub bezwaartermijn_einddatum: Option<String>,
    pub beoordelaar: String,
    /// Het bewijs van het besluit (peildatum, wetversies, parameters en
    /// outputs per component); None voor besluiten van vóór deze kolom.
    pub bewijs: Option<serde_json::Value>,
}

// Betaalopdracht-statussen. AANGEHOUDEN volgt uit art. 27 Wpp (geen
// rekening bekend); de overige zijn procesfasen richting het
// (gesimuleerde) betaalsysteem.
pub const BETAAL_AANGEMAAKT: &str = "AANGEMAAKT";
pub const BETAAL_AANGEHOUDEN: &str = "AANGEHOUDEN";
pub const BETAAL_UITBETAALD: &str = "UITBETAALD";

#[derive(Debug, Serialize)]
pub struct Betaalopdracht {
    pub id: String,
    pub besluit_id: String,
    /// Het dossier waar deze opdracht uit voortvloeit (via het besluit);
    /// de UI springt hiermee van betaalopdracht naar aanvraag en terug.
    pub aanvraag_id: String,
    pub partij_naam: String,
    pub bedrag: i64,
    /// Rekening van de rechtspersoon op het moment van verlening; leeg bij
    /// status AANGEHOUDEN (nog geen rekening bekend).
    pub iban: Option<String>,
    pub tenaamstelling: Option<String>,
    pub status: String,
    /// Uiterste betaaldatum (AWB 4:87), gezet bij de bekendmaking.
    pub betaaltermijn_einddatum: Option<String>,
    /// Moment van (gesimuleerde) uitbetaling.
    pub uitgevoerd_at: Option<String>,
    pub created_at: String,
}

fn row_to_aanvraag(row: &sqlx::sqlite::SqliteRow) -> Aanvraag {
    Aanvraag {
        id: row.get("id"),
        kvk_nummer: row.get("kvk_nummer"),
        partij_naam: row.get("partij_naam"),
        subsidiejaar: row.get("subsidiejaar"),
        componenten: serde_json::from_str(row.get::<String, _>("componenten").as_str())
            .unwrap_or_default(),
        parameters: serde_json::from_str(row.get::<String, _>("parameters").as_str())
            .unwrap_or(serde_json::Value::Null),
        status: row.get("status"),
        aanvraag_datum: row.get("aanvraag_datum"),
        beslistermijn_einddatum: row.get("beslistermijn_einddatum"),
        created_at: row.get("created_at"),
    }
}

fn row_to_besluit(row: &sqlx::sqlite::SqliteRow) -> Besluit {
    Besluit {
        id: row.get("id"),
        aanvraag_id: row.get("aanvraag_id"),
        subsidie_toegekend: row.get::<i64, _>("subsidie_toegekend") != 0,
        subsidiebedrag: row.get("subsidiebedrag"),
        componenten: serde_json::from_str(row.get::<String, _>("componenten").as_str())
            .unwrap_or_default(),
        motivering: row.get("motivering"),
        besluit_datum: row.get("besluit_datum"),
        bekendmaking_datum: row.get("bekendmaking_datum"),
        bezwaartermijn_startdatum: row.get("bezwaartermijn_startdatum"),
        bezwaartermijn_einddatum: row.get("bezwaartermijn_einddatum"),
        beoordelaar: row.get("beoordelaar"),
        bewijs: row
            .get::<Option<String>, _>("bewijs")
            .and_then(|s| serde_json::from_str(&s).ok()),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_aanvraag(
    pool: &SqlitePool,
    id: &str,
    kvk_nummer: &str,
    partij_naam: &str,
    subsidiejaar: i64,
    componenten_json: &str,
    parameters_json: &str,
    status: &str,
    aanvraag_datum: &str,
    beslistermijn_einddatum: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO aanvragen (id, kvk_nummer, partij_naam, subsidiejaar, componenten, parameters, status, aanvraag_datum, beslistermijn_einddatum)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(kvk_nummer)
    .bind(partij_naam)
    .bind(subsidiejaar)
    .bind(componenten_json)
    .bind(parameters_json)
    .bind(status)
    .bind(aanvraag_datum)
    .bind(beslistermijn_einddatum)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_aanvragen(
    pool: &SqlitePool,
    kvk_filter: Option<&str>,
) -> anyhow::Result<Vec<Aanvraag>> {
    let rows = match kvk_filter {
        Some(kvk) => {
            sqlx::query("SELECT * FROM aanvragen WHERE kvk_nummer = ? ORDER BY created_at DESC")
                .bind(kvk)
                .fetch_all(pool)
                .await?
        }
        None => {
            sqlx::query("SELECT * FROM aanvragen ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?
        }
    };
    Ok(rows.iter().map(row_to_aanvraag).collect())
}

pub async fn get_aanvraag(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Aanvraag>> {
    let row = sqlx::query("SELECT * FROM aanvragen WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_aanvraag))
}

pub async fn set_aanvraag_status(pool: &SqlitePool, id: &str, status: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE aanvragen SET status = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Componentsleutels van deze partij die in het subsidiejaar al bezet zijn:
/// in behandeling, of besloten met toekenning. Een afgewezen component mag
/// opnieuw worden aangevraagd.
/// Rauwe feiten per onderdeel uit de aanvragentabel en het
/// besluitenregister. Wat hiervan een nieuwe aanvraag blokkeert beslist de
/// wet (art. 13) — deze laag stelt alleen vast wat er gebeurd is.
#[derive(Debug, Clone, Copy, Default)]
pub struct OnderdeelFeiten {
    /// Er is voor dit onderdeel een aanvraag die nog op een besluit wacht.
    pub in_behandeling: bool,
    /// Er is voor dit onderdeel in het subsidiejaar subsidie toegekend.
    pub eerder_toegekend: bool,
    /// Er is voor dit onderdeel in het subsidiejaar eerder afgewezen.
    pub eerder_afgewezen: bool,
}

pub async fn onderdeel_feiten(
    pool: &SqlitePool,
    kvk: &str,
    subsidiejaar: i64,
) -> anyhow::Result<std::collections::HashMap<String, OnderdeelFeiten>> {
    let rows = sqlx::query(
        "SELECT a.id, a.componenten, a.status, b.componenten AS besluit_componenten
         FROM aanvragen a LEFT JOIN besluiten b ON b.aanvraag_id = a.id
         WHERE a.kvk_nummer = ? AND a.subsidiejaar = ?",
    )
    .bind(kvk)
    .bind(subsidiejaar)
    .fetch_all(pool)
    .await?;

    let mut feiten: std::collections::HashMap<String, OnderdeelFeiten> =
        std::collections::HashMap::new();
    for row in rows {
        let status: String = row.get("status");
        if status == crate::engine::STAGE_BEHANDELING {
            let componenten: Vec<Component> =
                serde_json::from_str(row.get::<String, _>("componenten").as_str())
                    .unwrap_or_default();
            for c in componenten {
                feiten.entry(c.key).or_default().in_behandeling = true;
            }
        } else if let Ok(uitkomsten) = serde_json::from_str::<Vec<ComponentUitkomst>>(
            row.get::<Option<String>, _>("besluit_componenten")
                .unwrap_or_default()
                .as_str(),
        ) {
            for u in uitkomsten {
                let feit = feiten.entry(u.component.key).or_default();
                if u.toegekend {
                    feit.eerder_toegekend = true;
                } else {
                    feit.eerder_afgewezen = true;
                }
            }
        }
    }
    Ok(feiten)
}

/// Som van de opgegeven ledentallen van alle aanvragen met een landelijke
/// component voor het subsidiejaar, voor zover de partij aan de ledeneis
/// voldoet (art. 6: minimaal duizend betalende leden). Dit is de noemer van
/// de ledencomponent (art. 14, onderdeel a): bij verlening werkt de Napp
/// met de opgaven die op dat moment binnen zijn; de definitieve verdeling
/// volgt bij de vaststelling (art. 18, buiten scope).
/// Eén landelijke opgave voor het ledenbudget: de bevroren kamerzetels en de
/// eigen opgaven van een aanvraag. Of de opgave meetelt in de noemer van de
/// ledencomponent bepaalt de wet (art. 6 jo. art. 14), niet deze query.
pub struct LandelijkeOpgave {
    pub zetels: i64,
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

/// Alle aanvragen voor het subsidiejaar die de landelijke component
/// bevatten, als ruwe opgaven. De wet beoordeelt ze stuk voor stuk.
pub async fn landelijke_opgaven(
    pool: &SqlitePool,
    subsidiejaar: i64,
) -> anyhow::Result<Vec<LandelijkeOpgave>> {
    let rows = sqlx::query("SELECT componenten, parameters FROM aanvragen WHERE subsidiejaar = ?")
        .bind(subsidiejaar)
        .fetch_all(pool)
        .await?;
    let mut opgaven = Vec::new();
    for row in rows {
        let componenten: Vec<Component> =
            serde_json::from_str(row.get::<String, _>("componenten").as_str()).unwrap_or_default();
        let Some(landelijk) = componenten.iter().find(|c| c.soort == "LANDELIJK") else {
            continue;
        };
        let parameters: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(row.get::<String, _>("parameters").as_str()).unwrap_or_default();
        opgaven.push(LandelijkeOpgave {
            zetels: landelijk.zetels,
            parameters,
        });
    }
    Ok(opgaven)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_besluit(
    pool: &SqlitePool,
    id: &str,
    aanvraag_id: &str,
    toegekend: bool,
    totaal: i64,
    componenten_json: &str,
    motivering: &str,
    besluit_datum: &str,
    beoordelaar: &str,
    bewijs_json: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO besluiten (id, aanvraag_id, subsidie_toegekend, subsidiebedrag, componenten, motivering, besluit_datum, beoordelaar, bewijs)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(aanvraag_id)
    .bind(toegekend as i64)
    .bind(totaal)
    .bind(componenten_json)
    .bind(motivering)
    .bind(besluit_datum)
    .bind(beoordelaar)
    .bind(bewijs_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_besluit_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Besluit>> {
    let row = sqlx::query("SELECT * FROM besluiten WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_besluit))
}

/// Vervang de uitkomst van een besluit na volledige heroverweging op
/// bezwaar (AWB 7:11): zelfde beschikking, herziene inhoud, nieuw bewijs.
pub async fn herzie_besluit(
    pool: &SqlitePool,
    id: &str,
    toegekend: bool,
    totaal: i64,
    componenten_json: &str,
    motivering: &str,
    bewijs_json: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE besluiten SET subsidie_toegekend = ?, subsidiebedrag = ?, componenten = ?,
         motivering = ?, bewijs = ? WHERE id = ?",
    )
    .bind(toegekend as i64)
    .bind(totaal)
    .bind(componenten_json)
    .bind(motivering)
    .bind(bewijs_json)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// De feitenbasis van een dossier bijwerken na gegrond bezwaar: de
/// gecorrigeerde eigen opgaven worden de parameters van de aanvraag.
pub async fn update_aanvraag_parameters(
    pool: &SqlitePool,
    id: &str,
    parameters_json: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE aanvragen SET parameters = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(parameters_json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_besluit_by_aanvraag(
    pool: &SqlitePool,
    aanvraag_id: &str,
) -> anyhow::Result<Option<Besluit>> {
    let row = sqlx::query("SELECT * FROM besluiten WHERE aanvraag_id = ?")
        .bind(aanvraag_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_besluit))
}

pub async fn set_bekendmaking(
    pool: &SqlitePool,
    besluit_id: &str,
    bekendmaking_datum: &str,
    start: &str,
    eind: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE besluiten SET bekendmaking_datum = ?, bezwaartermijn_startdatum = ?, bezwaartermijn_einddatum = ? WHERE id = ?",
    )
    .bind(bekendmaking_datum)
    .bind(start)
    .bind(eind)
    .bind(besluit_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_betaalopdracht(
    pool: &SqlitePool,
    id: &str,
    besluit_id: &str,
    partij_naam: &str,
    bedrag: i64,
    iban: Option<&str>,
    tenaamstelling: Option<&str>,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO betaalopdrachten (id, besluit_id, partij_naam, bedrag, iban, tenaamstelling, status)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(besluit_id)
    .bind(partij_naam)
    .bind(bedrag)
    .bind(iban)
    .bind(tenaamstelling)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

fn row_to_betaalopdracht(row: &sqlx::sqlite::SqliteRow) -> Betaalopdracht {
    Betaalopdracht {
        id: row.get("id"),
        besluit_id: row.get("besluit_id"),
        aanvraag_id: row.get("aanvraag_id"),
        partij_naam: row.get("partij_naam"),
        bedrag: row.get("bedrag"),
        iban: row.get("iban"),
        tenaamstelling: row.get("tenaamstelling"),
        status: row.get("status"),
        betaaltermijn_einddatum: row.get("betaaltermijn_einddatum"),
        uitgevoerd_at: row.get("uitgevoerd_at"),
        created_at: row.get("created_at"),
    }
}

// De aanvraag hoort bij elke opdracht (besluit_id is verplicht); de join
// levert het dossiernummer voor het heen-en-weer springen in de UI.
const BETAALOPDRACHT_SELECT: &str =
    "SELECT bo.*, b.aanvraag_id FROM betaalopdrachten bo JOIN besluiten b ON bo.besluit_id = b.id";

pub async fn list_betaalopdrachten(pool: &SqlitePool) -> anyhow::Result<Vec<Betaalopdracht>> {
    let rows = sqlx::query(&format!(
        "{BETAALOPDRACHT_SELECT} ORDER BY bo.created_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_betaalopdracht).collect())
}

pub async fn get_betaalopdracht(
    pool: &SqlitePool,
    id: &str,
) -> anyhow::Result<Option<Betaalopdracht>> {
    let row = sqlx::query(&format!("{BETAALOPDRACHT_SELECT} WHERE bo.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_betaalopdracht))
}

/// De betaalopdracht bij een besluit (hooguit één), voor het dossierbeeld.
pub async fn get_betaalopdracht_by_besluit(
    pool: &SqlitePool,
    besluit_id: &str,
) -> anyhow::Result<Option<Betaalopdracht>> {
    let row = sqlx::query(&format!("{BETAALOPDRACHT_SELECT} WHERE bo.besluit_id = ?"))
        .bind(besluit_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_betaalopdracht))
}

/// Aangehouden betaalopdrachten van een rechtspersoon, via het besluit en
/// de aanvraag (de betaalopdracht zelf kent geen KvK-nummer).
pub async fn aangehouden_betaalopdrachten(
    pool: &SqlitePool,
    kvk: &str,
) -> anyhow::Result<Vec<String>> {
    let ids = sqlx::query_scalar(
        "SELECT bo.id FROM betaalopdrachten bo
         JOIN besluiten b ON bo.besluit_id = b.id
         JOIN aanvragen a ON b.aanvraag_id = a.id
         WHERE a.kvk_nummer = ? AND bo.status = ?",
    )
    .bind(kvk)
    .bind(BETAAL_AANGEHOUDEN)
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

/// Activeer een aangehouden betaalopdracht nadat art. 27 de aanhouding
/// heeft opgeheven: rekening erbij, status AANGEMAAKT.
pub async fn activeer_betaalopdracht(
    pool: &SqlitePool,
    id: &str,
    iban: &str,
    tenaamstelling: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE betaalopdrachten SET status = ?, iban = ?, tenaamstelling = ?
         WHERE id = ? AND status = ?",
    )
    .bind(BETAAL_AANGEMAAKT)
    .bind(iban)
    .bind(tenaamstelling)
    .bind(id)
    .bind(BETAAL_AANGEHOUDEN)
    .execute(pool)
    .await?;
    Ok(())
}

/// Zet de uiterste betaaldatum (AWB 4:87) op de betaalopdrachten van een
/// besluit, bij de bekendmaking.
pub async fn set_betaaltermijn(
    pool: &SqlitePool,
    besluit_id: &str,
    einddatum: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE betaalopdrachten SET betaaltermijn_einddatum = ? WHERE besluit_id = ?")
        .bind(einddatum)
        .bind(besluit_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Markeer een aangemaakte betaalopdracht als (gesimuleerd) uitbetaald.
/// Geeft false terug wanneer de opdracht niet (meer) in de status
/// AANGEMAAKT staat.
pub async fn markeer_uitbetaald(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE betaalopdrachten SET status = ?, uitgevoerd_at = datetime('now')
         WHERE id = ? AND status = ?",
    )
    .bind(BETAAL_UITBETAALD)
    .bind(id)
    .bind(BETAAL_AANGEMAAKT)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Openbaar register: alleen bekendgemaakte besluiten.
#[derive(Debug, Serialize)]
pub struct RegisterEntry {
    pub partij_naam: String,
    pub subsidiejaar: i64,
    pub subsidie_toegekend: bool,
    pub subsidiebedrag: i64,
    pub aantal_componenten: i64,
    pub besluit_datum: String,
    pub bekendmaking_datum: String,
    pub bezwaartermijn_einddatum: Option<String>,
    pub componenten: Vec<ComponentUitkomst>,
}

pub async fn list_register(pool: &SqlitePool) -> anyhow::Result<Vec<RegisterEntry>> {
    let rows = sqlx::query(
        "SELECT a.partij_naam, a.subsidiejaar, b.subsidie_toegekend, b.subsidiebedrag,
                b.componenten, b.besluit_datum, b.bekendmaking_datum, b.bezwaartermijn_einddatum
         FROM besluiten b JOIN aanvragen a ON a.id = b.aanvraag_id
         WHERE b.bekendmaking_datum IS NOT NULL
         ORDER BY b.bekendmaking_datum DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let componenten: Vec<ComponentUitkomst> =
                serde_json::from_str(row.get::<String, _>("componenten").as_str())
                    .unwrap_or_default();
            RegisterEntry {
                partij_naam: row.get("partij_naam"),
                subsidiejaar: row.get("subsidiejaar"),
                subsidie_toegekend: row.get::<i64, _>("subsidie_toegekend") != 0,
                subsidiebedrag: row.get("subsidiebedrag"),
                aantal_componenten: componenten.len() as i64,
                besluit_datum: row.get("besluit_datum"),
                bekendmaking_datum: row.get("bekendmaking_datum"),
                bezwaartermijn_einddatum: row.get("bezwaartermijn_einddatum"),
                componenten,
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct Statistieken {
    pub aantal_aanvragen: i64,
    pub aantal_besluiten: i64,
    pub aantal_toegekend: i64,
    pub aantal_afgewezen: i64,
    pub totaal_toegekend_bedrag: i64,
    pub per_maand: Vec<MaandStat>,
}

#[derive(Debug, Serialize)]
pub struct MaandStat {
    pub maand: String,
    pub aantal_aanvragen: i64,
    pub toegekend_bedrag: i64,
}

pub async fn statistieken(pool: &SqlitePool) -> anyhow::Result<Statistieken> {
    let aantal_aanvragen: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aanvragen")
        .fetch_one(pool)
        .await?;
    let aantal_besluiten: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM besluiten")
        .fetch_one(pool)
        .await?;
    let aantal_toegekend: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM besluiten WHERE subsidie_toegekend = 1")
            .fetch_one(pool)
            .await?;
    let totaal_toegekend_bedrag: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(subsidiebedrag), 0) FROM besluiten WHERE subsidie_toegekend = 1",
    )
    .fetch_one(pool)
    .await?;

    let maand_rows = sqlx::query(
        "SELECT strftime('%Y-%m', a.aanvraag_datum) AS maand,
                COUNT(*) AS aantal,
                COALESCE(SUM(CASE WHEN b.subsidie_toegekend = 1 THEN b.subsidiebedrag ELSE 0 END), 0) AS toegekend
         FROM aanvragen a LEFT JOIN besluiten b ON b.aanvraag_id = a.id
         GROUP BY maand ORDER BY maand",
    )
    .fetch_all(pool)
    .await?;

    Ok(Statistieken {
        aantal_aanvragen,
        aantal_besluiten,
        aantal_toegekend,
        aantal_afgewezen: aantal_besluiten - aantal_toegekend,
        totaal_toegekend_bedrag,
        per_maand: maand_rows
            .iter()
            .map(|r| MaandStat {
                maand: r.get("maand"),
                aantal_aanvragen: r.get("aantal"),
                toegekend_bedrag: r.get("toegekend"),
            })
            .collect(),
    })
}
