//! Het opstarten uit een repo: één archiefverzoek, en daarna een wereld.
//!
//! Dit is het pad dat in een deployment gelopen wordt en dat lokaal nooit aan bod
//! komt: het wereldbestand én de regelingen staan in een (privé) repo, en dit
//! proces haalt ze bij het starten op met een token. Een wiremock-GitHub staat
//! hier in plaats van de echte, via de `GITHUB_API_BASE`-naad die
//! `regelrecht-github` daarvoor heeft.
//!
//! Er wordt op drie dingen gelet, en alle drie zijn het dingen die in een
//! container pas zouden opvallen:
//!
//! 1. het archief komt binnen en wordt uitgepakt, met de
//!    `{owner}-{repo}-{sha}/`-laag van GitHub eraf;
//! 2. twee bronnen in dezelfde repo op dezelfde ref kosten **één** verzoek —
//!    niet alleen goedkoper, ook het enige dat consistent is: een wereldbestand
//!    en de regelingen waarop het rust horen uit dezelfde momentopname te komen;
//!    en
//! 3. het token uit `CORPUS_AUTH_<SLUG>_TOKEN` gaat mee als `Authorization`,
//!    want zonder dat komt een privérepo niet binnen; en
//! 4. een pad dat in de opgehaalde momentopname niet bestaat, noemt zichzelf —
//!    ook als het het tweede pad uit dezelfde momentopname is.
//!
//! Eén test, want dit is één verhaal dat de env van het hele proces aanpast
//! (`GITHUB_API_BASE`); twee tests zouden daarop racen.

use std::io::Write;

use regelrecht_chrono_poc_web::sources::{resolve, Source};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Een wereldbestand zonder wetten: één bron-cel, dus geen engine en geen
/// regeling nodig. Wat hier getoetst wordt is het ophalen, niet de simulator.
const WORLD: &str = r"
clock:
  start: 2024-01-01
cells:
  - id: brp
    laws: []
    chronicles:
      - stream: relaties
        key: bsn
    lexostatus_definitions:
      - name: partnerschap
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true
";

/// Een gzipped tar zoals GitHub's tarball-endpoint hem geeft: alles onder één
/// map `{owner}-{repo}-{sha}/`.
fn tarball(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, body) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, body.as_bytes())
            .expect("tar-entry moet te schrijven zijn");
    }
    let tar = builder.into_inner().expect("tar moet af te ronden zijn");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).expect("gzip moet te schrijven zijn");
    gz.finish().expect("gzip moet af te ronden zijn")
}

#[tokio::test]
async fn een_wereld_uit_een_repo_kost_een_archiefverzoek_met_het_token() {
    let server = MockServer::start().await;
    // `regelrecht-github` leest deze variabele bij het bouwen van zijn client;
    // dat is de naad waarmee elke GitHub-aanroep in dit proces bij wiremock
    // uitkomt. De naam van de repo hieronder is verzonnen — in een publieke
    // repo hoort geen verwijzing naar een echt privécorpus.
    std::env::set_var("GITHUB_API_BASE", server.uri());
    std::env::set_var("CORPUS_AUTH_CORPUS_VOORBEELD_TOKEN", "geheim-token");

    Mock::given(method("GET"))
        .and(path("/repos/example-org/corpus-voorbeeld/tarball/main"))
        .and(header("authorization", "Bearer geheim-token"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball(&[
            (
                "example-org-corpus-voorbeeld-abc123/simulator/wereld.yaml",
                WORLD,
            ),
            (
                "example-org-corpus-voorbeeld-abc123/regulation/nl/wet/.gitkeep",
                "",
            ),
        ])))
        .mount(&server)
        .await;

    let world = Source::parse("github:example-org/corpus-voorbeeld@main:simulator/wereld.yaml")
        .expect("geldige wereldbron");
    let corpus = Source::parse("github:example-org/corpus-voorbeeld@main:regulation")
        .expect("geldige corpusbron");

    let resolved = resolve(&world, Some(&corpus), None)
        .await
        .unwrap_or_else(|e| panic!("het opstarten uit een repo moet slagen: {e}"));

    // Twee bronnen, één verzoek: het archief wordt hergebruikt.
    assert_eq!(
        server
            .received_requests()
            .await
            .map(|requests| requests.len()),
        Some(1),
        "twee bronnen uit dezelfde repo op dezelfde ref horen één archiefverzoek te kosten"
    );

    assert_eq!(resolved.definition.cells.len(), 1);
    assert_eq!(resolved.definition.cells[0].id, "brp");
    assert!(
        resolved.regulation_root.join("nl/wet").is_dir(),
        "de regelingenmap hoort uitgepakt te zijn zonder de archieflaag erboven, kreeg {}",
        resolved.regulation_root.display()
    );

    // De tijdelijke map leeft zolang `resolved` leeft, en verdwijnt daarna: dat
    // is wat het `fetched`-veld belooft.
    let root = resolved.regulation_root.clone();
    drop(resolved);
    assert!(
        !root.exists(),
        "de opgehaalde map hoort opgeruimd te worden, {} staat er nog",
        root.display()
    );

    // Een corpuspad dat in de momentopname niet bestaat, noemt zichzelf. Dit is
    // het tweede pad uit dezelfde momentopname en loopt dus langs het
    // hergebruikte archief; zonder de toets daar zou dit terugkomen als "zet
    // CHRONO_POC_CORPUS_SOURCE", van een variabele die wél gezet is.
    let mis = Source::parse("github:example-org/corpus-voorbeeld@main:regelingen")
        .expect("geldige corpusbron");
    let err = resolve(&world, Some(&mis), None)
        .await
        .expect_err("een corpuspad dat er niet in staat hoort het opstarten te laten falen");
    assert!(
        err.contains("regelingen") && err.contains("staat niet in"),
        "de melding hoort het pad in de repo te noemen: {err}"
    );

    std::env::remove_var("GITHUB_API_BASE");
    std::env::remove_var("CORPUS_AUTH_CORPUS_VOORBEELD_TOKEN");
}
