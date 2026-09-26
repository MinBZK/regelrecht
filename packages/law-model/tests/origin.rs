//! `origin` on a parameter and `origins` on an article (RFC-043): metadata for
//! a process runtime and an editor. The engine does not read it, so a law
//! without it parses as before, a law with it survives a round trip, and a
//! law with an invalid one still loads: the runtime that reads it reports it.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use regelrecht_law_model::{ArticleBasedLaw, Declared, OriginRole, OriginValue};

const WET: &str = r#"
$id: een_regeling
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De aanvraag bevat de naam van de aanvrager.
    machine_readable:
      execution:
        parameters:
          - name: bevat_naam
            type: boolean
            required: false
            origin:
              waarde: BELANGHEBBENDE
              grondslag: een_regeling#1 lid 1
          - name: is_ingeschreven
            type: boolean
            origin: {waarde: REGISTER, register: een_registerwet, grondslag: een_regeling#2}
          - name: zonder_origin
            type: boolean
        output:
          - name: volledig
            type: boolean
        actions:
          - output: volledig
            value: $bevat_naam
"#;

const BELEID: &str = r#"
$id: een_beleid
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De instantie haalt de inschrijving zelf op.
    machine_readable:
      origins:
        - regulation: een_regeling
          parameter: bevat_naam
          origin: {waarde: REGISTER, register: een_registerwet, grondslag: een_beleid#4 lid 1}
"#;

fn parse(yaml: &str) -> ArticleBasedLaw {
    serde_yaml_ng::from_str(yaml).expect("model should parse")
}

#[test]
fn origin_on_a_parameter() {
    let law = parse(WET);
    let p = law.articles[0].get_parameters();
    let o = p[0]
        .origin
        .as_ref()
        .and_then(Declared::as_valid)
        .expect("origin");
    assert_eq!(o.waarde, OriginValue::Belanghebbende);
    assert_eq!(o.grondslag, "een_regeling#1 lid 1");
    assert_eq!(o.register, None);
    let r = p[1]
        .origin
        .as_ref()
        .and_then(Declared::as_valid)
        .expect("origin");
    assert_eq!(r.waarde, OriginValue::Register);
    assert_eq!(r.register.as_deref(), Some("een_registerwet"));
    assert!(p[2].origin.is_none());
}

#[test]
fn origins_on_an_article() {
    let law = parse(BELEID);
    let o = law.articles[0]
        .machine_readable
        .as_ref()
        .and_then(|m| m.origins.as_ref())
        .expect("origins");
    assert_eq!(o.len(), 1);
    let o = o[0].as_valid().expect("valid");
    assert_eq!(o.regulation, "een_regeling");
    assert_eq!(o.parameter, "bevat_naam");
    assert_eq!(o.origin.waarde, OriginValue::Register);
}

/// An invalid origin does not stop the law from loading; the reason comes
/// from [`Declared::valid`].
#[test]
fn an_invalid_origin_is_kept_with_its_reason() {
    let law = parse(&WET.replace("waarde: BELANGHEBBENDE", "waarde: KADER"));
    let p = law.articles[0].get_parameters();
    let fout = p[0].origin.as_ref().expect("origin").valid().unwrap_err();
    assert!(fout.contains("unknown variant `KADER`"), "{fout}");

    // A misspelled key is not silently dropped.
    let law = parse(&WET.replace("register: een_registerwet", "registr: een_registerwet"));
    let fout = law.articles[0].get_parameters()[1]
        .origin
        .as_ref()
        .expect("origin")
        .valid()
        .unwrap_err();
    assert!(fout.contains("unknown field `registr`"), "{fout}");

    // The same for an override in policy.
    let law = parse(&BELEID.replace("parameter: bevat_naam", "parameter: 12"));
    let o = &law.articles[0]
        .machine_readable
        .as_ref()
        .unwrap()
        .origins
        .as_ref()
        .unwrap()[0];
    assert!(o.valid().is_err());
}

#[test]
fn the_role_of_a_period() {
    let law = parse(&WET.replace(
        "grondslag: een_regeling#1 lid 1",
        "grondslag: een_regeling#1 lid 1\n              rol: TIJDVAK",
    ));
    let o = law.articles[0].get_parameters()[0]
        .origin
        .as_ref()
        .and_then(Declared::as_valid)
        .expect("origin");
    assert_eq!(o.rol, Some(OriginRole::Tijdvak));
    assert_eq!(OriginRole::Tijdvak.as_str(), "TIJDVAK");
}

#[test]
fn origin_survives_a_round_trip() {
    let law = parse(WET);
    let yaml = serde_yaml_ng::to_string(&law).expect("serialize");
    assert!(yaml.contains("waarde: BELANGHEBBENDE"), "{yaml}");
    assert!(!yaml.contains("origins"), "{yaml}");
    let terug = parse(&yaml);
    assert_eq!(
        terug.articles[0].get_parameters(),
        law.articles[0].get_parameters()
    );
}
