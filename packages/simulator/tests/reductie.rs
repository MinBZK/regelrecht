//! De uitleg bij een lexostatus: welke grammen gelezen zijn, en hoe.
//!
//! Vier gevallen, en dat zijn ze alle vier: het kroniekfilter dat de laatste
//! vastlegging leest, de som over een kroniek, de wetsvorm die de eigen engine
//! laat rekenen, en het filter dat niets aantreft. De eerste drie moeten de
//! grammen noemen waaruit het antwoord komt; het vierde moet zeggen wat het
//! zocht en wat er wél lag.
//!
//! De wereld staat hier in het bestand en niet in `worlds/`: die wereld is er
//! om bespeeld te worden, deze om een eigenschap van de opstelling te meten.
//! Wat hier nodig is — twee betalingen om op te tellen, een `where` waar iets
//! net buiten valt, een moment vóór de eerste vastlegging — zou daar een casus
//! verzinnen die niemand speelt.

use regelrecht_simulator::{
    regulation_root, GebruiktGram, InputHerkomst, Kroniekfilter, Lexostatus, Reductie,
    ReductieVorm, Regel, Value, Wetsvorm, World, WorldDefinition,
};
use std::collections::BTreeMap;

/// Een testpersoon. Een van de nummers die het BSN-stelsel voor tests
/// vrijhoudt, net als in de scenario's.
const BSN: &str = "999993653";

/// De wereld van deze meting: een bron-cel met twee kronieken, en een cel met
/// een engine die over haar eigen relatiefeiten rekent.
fn world_yaml() -> String {
    format!(
        r"
clock:
  start: 2025-01-01

cells:
  - id: register
    laws: []
    chronicles:
      - stream: relaties
        key: bsn
        events:
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: register
            grondslag: eigen registratie
            op_moment: 2023-01-01
            fields:
              bsn: '{BSN}'
              partnerschap_type: HUWELIJK
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: register
            grondslag: eigen registratie
            op_moment: 2024-07-01
            fields:
              bsn: '{BSN}'
              partnerschap_type: GEEN

      - stream: betalingen
        key: zaakkenmerk
        events:
          - name: betaling_gedaan
            intake: betaling
            recording_actor: register
            grondslag: betalingsopdracht
            op_moment: 2024-03-01
            fields:
              zaakkenmerk: zaak/1
              bedrag: 100
          - name: betaling_gedaan
            intake: betaling
            recording_actor: register
            grondslag: betalingsopdracht
            op_moment: 2024-06-01
            fields:
              zaakkenmerk: zaak/1
              bedrag: 250
          - name: betaling_gedaan
            intake: betaling
            recording_actor: register
            grondslag: betalingsopdracht
            op_moment: 2024-09-01
            fields:
              zaakkenmerk: zaak/2
              bedrag: 900

    lexostatus_definitions:
      - name: partnerschap
        doc: de laatste relatievastlegging op of vóór het gevraagde moment
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true

      - name: laatste_huwelijk
        doc: de laatste vastlegging die een huwelijk was
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true
          where:
            partnerschap_type: HUWELIJK

      - name: laatste_geregistreerd_partnerschap
        doc: de laatste vastlegging die een geregistreerd partnerschap was
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true
          where:
            partnerschap_type: GEREGISTREERD_PARTNERSCHAP

      - name: betaald_tot_nu_toe
        doc: wat er op deze zaak betaald is, opgeteld
        inputs:
          - name: zaakkenmerk
            type: string
        outputs:
          - bedrag
        reduction:
          chronicle: betalingen
          key: zaakkenmerk
          sum: bedrag

  - id: uitvoerder
    laws:
      - wet_op_de_zorgtoeslag
      - algemene_wet_inkomensafhankelijke_regelingen
      - regeling_standaardpremie
    chronicles:
      - stream: relaties
        key: bsn
        events:
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: uitvoerder
            grondslag: melding uit de basisregistratie personen
            op_moment: 2023-01-01
            fields:
              bsn: '{BSN}'
              partnerschap_type: GEEN

      - stream: inkomensleveringen
        key: bsn
        events:
          - name: inkomenslevering
            intake: levering
            recording_actor: uitvoerder
            grondslag: jaarlijkse inkomenslevering
            op_moment: 2024-11-15
            fields:
              bsn: '{BSN}'
              is_verzekerde: true
              verzamelinkomen: 79547
              buitenlands_inkomen: 0
              vermogen: 0

    lexostatus_definitions:
      - name: zorgtoeslag_rechtstoestand
        doc: de zorgtoeslagpositie op de feiten die deze cel kent
        inputs:
          - name: bsn
            type: string
        outputs:
          - heeft_recht_op_zorgtoeslag
        reduction:
          regulation: wet_op_de_zorgtoeslag
          output: heeft_recht_op_zorgtoeslag
          parameters:
            bsn: $bsn

  # Een wetsvorm waarvan de inputs niet uit een kroniek komen maar uit de vraag.
  # `test_nabetaling` is een testregeling naast het corpus (zie
  # `fixtures/regulation/`): haar twee inputs dragen een lege `source`, dus
  # wie ze heeft, geeft ze mee.
  - id: rekenaar
    laws:
      - test_nabetaling
    chronicles: []

    lexostatus_definitions:
      - name: nog_te_betalen
        doc: het verschil tussen wat is toegekend en wat is betaald
        inputs:
          - name: toegekend
            type: number
          - name: betaald
            type: number
        outputs:
          - nog_te_betalen
        reduction:
          regulation: test_nabetaling
          output: nog_te_betalen
          parameters:
            toegekend_bedrag: $toegekend
            betaald_bedrag: $betaald
"
    )
}

fn world() -> World {
    let definition = WorldDefinition::from_yaml(&world_yaml())
        .unwrap_or_else(|e| panic!("het wereldbestand van deze test hoort te lezen: {e}"));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("de wereld van deze test hoort op te tuigen: {e}"))
}

fn moment(text: &str) -> chrono::NaiveDate {
    text.parse()
        .unwrap_or_else(|e| panic!("{text} hoort een datum te zijn: {e}"))
}

fn param(name: &str, value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([(name.to_string(), Value::String(value.to_string()))])
}

/// Eén vraag aan deze wereld.
fn ask(cell: &str, lexostatus: &str, params: &BTreeMap<String, Value>, op: &str) -> Lexostatus {
    world()
        .reduce(cell, lexostatus, params, moment(op))
        .unwrap_or_else(|e| panic!("{cell}.{lexostatus} hoort te antwoorden: {e}"))
}

/// De uitleg bij een antwoord. Ze hoort er altijd te zijn.
fn uitleg(answer: &Lexostatus) -> &Reductie {
    answer.reductie.as_ref().unwrap_or_else(|| {
        panic!("elk antwoord van een reductie hoort te zeggen hoe het tot stand kwam")
    })
}

/// Het kroniekfilter uit een uitleg.
fn filter(reductie: &Reductie) -> &Kroniekfilter {
    match &reductie.vorm {
        ReductieVorm::Kroniekfilter(filter) => filter,
        ReductieVorm::Wetsvorm(_) | ReductieVorm::Openstaand(_) => {
            panic!("verwachtte een kroniekfilter")
        }
    }
}

/// De wetsvorm uit een uitleg.
fn wetsvorm(reductie: &Reductie) -> &Wetsvorm {
    match &reductie.vorm {
        ReductieVorm::Wetsvorm(wetsvorm) => wetsvorm,
        ReductieVorm::Kroniekfilter(_) | ReductieVorm::Openstaand(_) => {
            panic!("verwachtte een wetsvorm")
        }
    }
}

/// Hoe een gelezen gram in een assertie heet.
fn beschrijf(gram: &GebruiktGram) -> String {
    format!("{} ({})", gram.gram.id, gram.op_moment)
}

#[test]
fn het_filter_noemt_de_vastlegging_waaruit_het_antwoord_komt() {
    let answer = ask("register", "partnerschap", &param("bsn", BSN), "2025-01-01");
    let reductie = uitleg(&answer);
    let filter = filter(reductie);

    assert_eq!(filter.chronicle, "relaties");
    assert_eq!(filter.key, "bsn");
    assert_eq!(filter.key_value, Value::String(BSN.to_string()));
    assert!(
        filter.conditions.is_empty(),
        "dit filter heeft geen `where`"
    );
    assert_eq!(filter.regel, Regel::Laatste);
    assert_eq!(filter.op_moment, moment("2025-01-01"));

    // Eén gram, en het is de tweede vastlegging: de laatste op of vóór het
    // moment. Zou de uitleg de eerste noemen, dan zou ze een antwoord verklaren
    // dat er niet staat.
    assert_eq!(
        reductie.grammen.iter().map(beschrijf).collect::<Vec<_>>(),
        ["register|relaties|1 (2024-07-01)"],
        "de uitleg hoort precies de gelezen vastlegging te noemen"
    );
    let gram = &reductie.grammen[0];
    assert_eq!(gram.gram.name, "relatie_gewijzigd");
    assert_eq!(gram.volgnummer, 1);
    assert_eq!(gram.bijdrage, None, "een filter telt niets op");
    assert_eq!(reductie.gemist, None, "er is wél iets vastgesteld");
}

/// Een vraag over een eerder moment leest een ander gram, en zegt dat ook.
#[test]
fn het_filter_van_toen_noemt_het_gram_van_toen() {
    let answer = ask("register", "partnerschap", &param("bsn", BSN), "2024-01-01");
    let reductie = uitleg(&answer);

    assert_eq!(
        reductie.grammen.iter().map(beschrijf).collect::<Vec<_>>(),
        ["register|relaties|0 (2023-01-01)"],
        "op een moment vóór de tweede vastlegging gold de eerste"
    );
    assert_eq!(filter(reductie).op_moment, moment("2024-01-01"));
}

/// Een `where` staat in de uitleg, en het gram is dat van de voorwaarde.
#[test]
fn het_filter_noemt_de_voorwaarde_waarop_het_koos() {
    let answer = ask(
        "register",
        "laatste_huwelijk",
        &param("bsn", BSN),
        "2025-01-01",
    );
    let reductie = uitleg(&answer);

    assert_eq!(
        filter(reductie).conditions,
        BTreeMap::from([(
            "partnerschap_type".to_string(),
            Value::String("HUWELIJK".to_string())
        )]),
        "de voorwaarde hoort in de uitleg te staan; zonder haar is niet te zien \
         waarom de laatste vastlegging niet gelezen is"
    );
    assert_eq!(
        reductie.grammen.iter().map(beschrijf).collect::<Vec<_>>(),
        ["register|relaties|0 (2023-01-01)"],
        "de laatste vastlegging die aan de voorwaarde voldeed, en niet de laatste"
    );
}

#[test]
fn de_som_noemt_elk_meegeteld_gram_met_zijn_bijdrage() {
    let answer = ask(
        "register",
        "betaald_tot_nu_toe",
        &param("zaakkenmerk", "zaak/1"),
        "2025-01-01",
    );
    let reductie = uitleg(&answer);

    assert_eq!(
        filter(reductie).regel,
        Regel::Som {
            field: "bedrag".to_string()
        },
        "de regel hoort te zeggen welk veld opgeteld is"
    );

    // Beide betalingen op deze zaak, elk met wat ze bijdroeg. Een som waarvan
    // alleen de uitkomst te zien is, valt niet na te rekenen.
    assert_eq!(
        reductie.grammen.iter().map(beschrijf).collect::<Vec<_>>(),
        [
            "register|betalingen|0 (2024-03-01)",
            "register|betalingen|1 (2024-06-01)"
        ],
        "elk meegeteld gram hoort in de uitleg te staan"
    );
    let bijdragen: Vec<Option<&Value>> = reductie
        .grammen
        .iter()
        .map(|gram| gram.bijdrage.as_ref())
        .collect();
    assert_eq!(
        bijdragen,
        [Some(&Value::Int(100)), Some(&Value::Int(250))],
        "elk meegeteld gram hoort te zeggen wát het bijdroeg, en het bedrag hoort \
         dat van die vastlegging te zijn"
    );

    // En de bijdragen zijn de som: dát is wat "na te rekenen" betekent. Een
    // uitleg waarvan de getallen niet op de uitkomst uitkomen, verklaart een
    // ander antwoord dan het gegeven antwoord.
    let uitkomst = answer
        .values()
        .unwrap_or_else(|| panic!("op deze zaak is betaald, dus hier hoort een bedrag te staan"))
        .get("bedrag")
        .cloned();
    assert_eq!(
        uitkomst,
        Some(Value::Int(350)),
        "100 + 250 is wat de uitleg optelt, en dat hoort de uitkomst te zijn"
    );

    // En het gram van de andere zaak doet niet mee: de sleutel is wat de zaken
    // scheidt, en een uitleg die te veel noemt, verklaart een ander bedrag.
    assert!(
        !reductie.grammen.iter().any(|gram| gram.volgnummer == 2),
        "de betaling op de andere zaak hoort er niet in te staan"
    );
}

#[test]
fn de_wetsvorm_noemt_de_regeling_haar_versie_en_de_herkomst_van_elke_input() {
    let answer = ask(
        "uitvoerder",
        "zorgtoeslag_rechtstoestand",
        &param("bsn", BSN),
        "2025-01-01",
    );
    let reductie = uitleg(&answer);
    let wetsvorm = wetsvorm(reductie);

    assert_eq!(wetsvorm.regulation, "wet_op_de_zorgtoeslag");
    assert_eq!(wetsvorm.output, "heeft_recht_op_zorgtoeslag");
    assert_eq!(
        wetsvorm.regulation_valid_from.as_deref(),
        Some("2025-01-01"),
        "zonder de versie is niet te zien onder welk recht gerekend is"
    );
    assert_eq!(wetsvorm.op_moment, moment("2025-01-01"));
    assert!(
        !wetsvorm.inputs.is_empty(),
        "een uitvoering zonder inputs verklaart niets"
    );

    // De kronieken die als databron klaarstonden, elk met het gram dat op dit
    // moment over deze persoon gold. Dat gram weet de engine niet: zij ziet één
    // record per onderwerp, want de tijdreductie is er dan al overheen gegaan.
    assert_eq!(
        reductie.grammen.iter().map(beschrijf).collect::<Vec<_>>(),
        ["uitvoerder|inkomensleveringen|0 (2024-11-15)"],
        "de uitleg hoort de grammen te noemen waaruit de engine las"
    );
    let uit_de_kroniek: Vec<&str> = wetsvorm
        .inputs
        .iter()
        .filter_map(|input| match &input.herkomst {
            InputHerkomst::EigenKroniek { gram, .. } => gram.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(
        uit_de_kroniek,
        ["uitvoerder|inkomensleveringen|0"],
        "een input uit een kroniek hoort naar het gram te wijzen dat hem droeg"
    );
    // Wat de wet bij een andere regeling ophaalde, heet zo en niet "kroniek": de
    // tiers van de resolutievolgorde horen uit elkaar te blijven.
    assert!(
        wetsvorm
            .inputs
            .iter()
            .any(|input| matches!(&input.herkomst, InputHerkomst::Regeling { .. })),
        "deze wet leunt op andere regelingen; dat hoort te zien te zijn: {:?}",
        wetsvorm.inputs
    );
    assert!(
        !wetsvorm
            .inputs
            .iter()
            .any(|input| matches!(&input.herkomst, InputHerkomst::Cel { .. })),
        "een reductie kan geen andere cel bereiken, dus geen enkele input hoort \
         er een te noemen: {:?}",
        wetsvorm.inputs
    );
}

/// Een input die de vraag zelf meebracht, heet naar de parameter van de vraag.
///
/// De naam waaronder de *consument* hem kende, en niet de naam die de regeling
/// hem geeft: wie zijn eigen parameter niet terugziet, kan de uitleg niet aan
/// zijn vraag koppelen.
#[test]
fn een_input_uit_de_vraag_noemt_de_parameter_van_de_vraag() {
    let params = BTreeMap::from([
        ("toegekend".to_string(), Value::Int(50_000)),
        ("betaald".to_string(), Value::Int(20_000)),
    ]);
    let answer = ask("rekenaar", "nog_te_betalen", &params, "2025-01-01");
    let reductie = uitleg(&answer);
    let wetsvorm = wetsvorm(reductie);

    assert_eq!(
        wetsvorm
            .inputs
            .iter()
            .map(|input| format!("{} <- {:?}", input.name, input.herkomst))
            .collect::<Vec<_>>(),
        [
            "betaald_bedrag <- Parameter { parameter: Some(\"betaald\") }",
            "toegekend_bedrag <- Parameter { parameter: Some(\"toegekend\") }",
        ],
        "beide inputs kwamen uit de vraag, elk onder de naam die de vraag ervoor \
         gebruikte"
    );
    assert!(
        reductie.grammen.is_empty(),
        "deze cel houdt geen kronieken, dus er valt geen gram te noemen"
    );
    assert_eq!(
        wetsvorm.regulation_valid_from.as_deref(),
        Some("2024-01-01")
    );
}

#[test]
fn niets_vastgesteld_zegt_wat_er_gezocht_is_en_wat_er_wel_lag() {
    // Vóór de eerste vastlegging: over deze persoon was hier toen niets bekend.
    let answer = ask("register", "partnerschap", &param("bsn", BSN), "2022-12-31");
    let reductie = uitleg(&answer);
    let filter = filter(reductie);

    assert!(
        answer.not_established().is_some(),
        "op dit moment hoort er niets vastgesteld te zijn"
    );
    assert_eq!(filter.chronicle, "relaties");
    assert_eq!(filter.key_value, Value::String(BSN.to_string()));
    assert_eq!(filter.op_moment, moment("2022-12-31"));
    assert!(
        reductie.grammen.is_empty(),
        "er is niets gelezen, en dat is precies wat er te melden valt"
    );

    let gemist = reductie
        .gemist
        .unwrap_or_else(|| panic!("niets vastgesteld hoort te zeggen wat er wél lag"));
    assert_eq!(gemist.in_de_stroom, 2);
    assert_eq!(
        gemist.na_het_moment, 2,
        "beide vastleggingen liggen ná het gevraagde moment: te vroeg gevraagd, \
         en niet de verkeerde zaak"
    );
    assert_eq!(gemist.andere_sleutel, 0);
    assert_eq!(gemist.buiten_de_voorwaarden, 0);
}

/// Dezelfde stroom, een ander onderwerp: dan is het de sleutel die telt.
#[test]
fn niets_vastgesteld_onderscheidt_de_verkeerde_zaak_van_het_verkeerde_moment() {
    let answer = ask(
        "register",
        "betaald_tot_nu_toe",
        &param("zaakkenmerk", "zaak/3"),
        "2025-01-01",
    );
    let gemist = uitleg(&answer)
        .gemist
        .unwrap_or_else(|| panic!("niets vastgesteld hoort te zeggen wat er wél lag"));

    assert_eq!(gemist.in_de_stroom, 3);
    assert_eq!(gemist.na_het_moment, 0);
    assert_eq!(
        gemist.andere_sleutel, 3,
        "er ligt van alles in deze stroom, maar niets over deze zaak — en dat is \
         iets anders dan een lege stroom"
    );
}

/// Een voorwaarde die niets overlaat, telt apart.
///
/// Over deze persoon ligt er van alles, op tijd en onder de juiste sleutel; het
/// is de `where` waarop het afvalt. Dat is een derde reden om niets vast te
/// stellen, en wie hem niet van de andere twee kan onderscheiden, gaat op de
/// verkeerde plek zoeken.
#[test]
fn niets_vastgesteld_telt_wat_op_de_voorwaarde_afviel() {
    let answer = ask(
        "register",
        "laatste_geregistreerd_partnerschap",
        &param("bsn", BSN),
        "2025-01-01",
    );
    let gemist = uitleg(&answer)
        .gemist
        .unwrap_or_else(|| panic!("niets vastgesteld hoort te zeggen wat er wél lag"));

    assert_eq!(gemist.in_de_stroom, 2);
    assert_eq!(gemist.na_het_moment, 0);
    assert_eq!(gemist.andere_sleutel, 0);
    assert_eq!(
        gemist.buiten_de_voorwaarden, 2,
        "beide vastleggingen gaan over deze persoon en liggen op tijd; ze vielen \
         af op de voorwaarde, en dat is iets anders dan een lege stroom of een \
         te vroege vraag"
    );
}

/// De sleutel valt af vóór de voorwaarde eraan toekomt.
///
/// De drie tellingen sluiten elkaar uit — een gram telt één keer, bij de eerste
/// reden waarop het afviel. Zonder die volgorde zou een gram van een ander
/// onderwerp ook "buiten de voorwaarden" heten, en dan wijzen de getallen twee
/// kanten op.
#[test]
fn niets_vastgesteld_laat_de_sleutel_voor_de_voorwaarde_afvallen() {
    let answer = ask(
        "register",
        "laatste_huwelijk",
        &param("bsn", "000000000"),
        "2025-01-01",
    );
    let gemist = uitleg(&answer)
        .gemist
        .unwrap_or_else(|| panic!("niets vastgesteld hoort te zeggen wat er wél lag"));

    assert_eq!(
        gemist.andere_sleutel, 2,
        "een andere persoon: de sleutel valt af vóór de voorwaarde eraan toekomt"
    );
    assert_eq!(gemist.buiten_de_voorwaarden, 0);
}

/// De uitleg hangt niet van de wandklok af.
///
/// Twee keer dezelfde vraag aan twee keer dezelfde wereld levert dezelfde
/// uitleg. Zonder deze eigenschap is het blok geen contract maar een verslag.
#[test]
fn dezelfde_vraag_levert_dezelfde_uitleg() {
    let params = param("zaakkenmerk", "zaak/1");
    let eerst = ask("register", "betaald_tot_nu_toe", &params, "2025-01-01");
    let opnieuw = ask("register", "betaald_tot_nu_toe", &params, "2025-01-01");
    assert_eq!(uitleg(&eerst), uitleg(&opnieuw));
}
