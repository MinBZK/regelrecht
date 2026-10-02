//! The reversal the BDD runner applies with `BDD_REVERSE_ACTIONS=1`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

// The runner reads `enabled`; this test only exercises the reversal.
#[allow(dead_code)]
#[path = "common/reverse_actions.rs"]
mod reverse_actions;

use reverse_actions::reversed_actions;

fn order(law: &str) -> Vec<(String, i64)> {
    let reversed: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&reversed_actions(law).unwrap()).unwrap();
    reversed["articles"][0]["machine_readable"]["execution"]["actions"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["output"].as_str().unwrap().to_string(),
                a["value"].as_i64().unwrap_or(0),
            )
        })
        .collect()
}

#[test]
fn the_outputs_are_reversed() {
    let law = "articles:\n  - number: '1'\n    machine_readable:\n      execution:\n        actions:\n          - output: a\n            value: 1\n          - output: b\n            value: 2\n          - output: c\n            value: 3\n";
    assert_eq!(
        order(law),
        vec![
            ("c".to_string(), 3),
            ("b".to_string(), 2),
            ("a".to_string(), 1)
        ]
    );
}

#[test]
fn the_assignments_of_one_output_keep_their_order() {
    let law = "articles:\n  - number: '1'\n    machine_readable:\n      execution:\n        actions:\n          - output: x\n            value: 1\n          - output: y\n            value: $x\n          - output: x\n            value: 2\n";
    assert_eq!(
        order(law),
        vec![
            ("y".to_string(), 0),
            ("x".to_string(), 1),
            ("x".to_string(), 2)
        ]
    );
}
