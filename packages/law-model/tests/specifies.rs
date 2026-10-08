//! `specifies` on a parameter: the parameter of a general law that this one
//! is the specific form of (lex specialis). Metadata for a process runtime;
//! the engine does not read it. A law without it parses as before, and a key
//! the model does not know is still ignored on a parameter.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use regelrecht_law_model::{ArticleBasedLaw, Specifies};

const WET: &str = r#"
$id: een_bijzondere_regeling
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De aanvraag bevat de statutaire naam van de partij.
    machine_readable:
      execution:
        parameters:
          - name: statutaire_naam
            type: string
            specifies:
              regulation: een_algemene_wet
              article: '4:2'
              parameter: naam_aanvrager
          - name: zonder_specifies
            type: string
          - name: met_onbekende_sleutel
            type: string
            onbekend: wat dan ook
        output:
          - name: naam
            type: string
        actions:
          - output: naam
            value: $statutaire_naam
"#;

fn parse(yaml: &str) -> ArticleBasedLaw {
    serde_yaml_ng::from_str(yaml).expect("model should parse")
}

#[test]
fn specifies_on_a_parameter() {
    let law = parse(WET);
    let p = law.articles[0].get_parameters();
    assert_eq!(
        p[0].specifies,
        Some(Specifies {
            regulation: "een_algemene_wet".into(),
            article: "4:2".into(),
            parameter: "naam_aanvrager".into(),
        })
    );
    assert_eq!(p[1].specifies, None);
}

#[test]
fn an_unknown_key_on_a_parameter_is_still_ignored() {
    let law = parse(WET);
    let p = &law.articles[0].get_parameters()[2];
    assert_eq!(p.name, "met_onbekende_sleutel");
    assert_eq!(p.specifies, None);
}

#[test]
fn specifies_survives_a_round_trip_and_is_left_out_when_absent() {
    let law = parse(WET);
    let yaml = serde_yaml_ng::to_string(&law).unwrap();
    assert_eq!(
        parse(&yaml).articles[0].get_parameters(),
        law.articles[0].get_parameters()
    );
    let absent = serde_yaml_ng::to_string(&law.articles[0].get_parameters()[1]).unwrap();
    assert!(!absent.contains("specifies:"), "{absent}");
}

#[test]
fn specifies_names_all_three_or_the_law_does_not_parse() {
    let incomplete = WET.replace("              parameter: naam_aanvrager\n", "");
    assert!(serde_yaml_ng::from_str::<ArticleBasedLaw>(&incomplete).is_err());
}
