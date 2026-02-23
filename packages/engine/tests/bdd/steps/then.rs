//! Then step definitions
//!
//! Steps that verify outcomes and assertions.

use cucumber::then;
use regelrecht_engine::Value;

use crate::world::RegelrechtWorld;

// Note: Some test failures are expected due to known engine limitations:
// - Bijstand tests: Rust engine doesn't have "uitvoerder context" mechanism for gedragscategorie
// - Erfgrensbeplanting without verordening: Rust engine doesn't support delegation defaults yet

// =============================================================================
// Bijstand steps
// =============================================================================

#[then("the citizen has the right to bijstand")]
fn assert_has_right_to_bijstand(world: &mut RegelrechtWorld) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let has_right = world.get_output("heeft_recht_op_bijstand");
    assert!(
        matches!(has_right, Some(Value::Bool(true))),
        "Expected heeft_recht_op_bijstand to be true, got {:?}",
        has_right
    );
}

#[then("the citizen does not have the right to bijstand")]
fn assert_no_right_to_bijstand(world: &mut RegelrechtWorld) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let has_right = world.get_output("heeft_recht_op_bijstand");
    assert!(
        matches!(has_right, Some(Value::Bool(false))),
        "Expected heeft_recht_op_bijstand to be false, got {:?}",
        has_right
    );
}

#[then(regex = r#"^the uitkering_bedrag is "(\d+)" eurocent$"#)]
fn assert_uitkering_bedrag(world: &mut RegelrechtWorld, expected: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let expected_amount: i64 = expected
        .parse()
        .unwrap_or_else(|_| panic!("Invalid eurocent value: {}", expected));

    let actual = world.get_output("uitkering_bedrag");
    match actual {
        Some(Value::Int(n)) => {
            assert_eq!(
                *n, expected_amount,
                "Expected uitkering_bedrag to be {} eurocent, got {}",
                expected_amount, n
            );
        }
        Some(Value::Float(f)) => {
            let actual_int = f.round() as i64;
            assert_eq!(
                actual_int, expected_amount,
                "Expected uitkering_bedrag to be {} eurocent, got {} (rounded from {})",
                expected_amount, actual_int, f
            );
        }
        _ => panic!("Expected uitkering_bedrag to be a number, got {:?}", actual),
    }
}

#[then(regex = r#"^the reden_afwijzing contains "([^"]+)"$"#)]
fn assert_reden_afwijzing_contains(world: &mut RegelrechtWorld, expected_text: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let reden = world.get_output("reden_afwijzing");
    match reden {
        Some(Value::String(s)) => {
            assert!(
                s.to_lowercase().contains(&expected_text.to_lowercase()),
                "Expected reden_afwijzing to contain '{}', got '{}'",
                expected_text,
                s
            );
        }
        _ => panic!("Expected reden_afwijzing to be a string, got {:?}", reden),
    }
}

// =============================================================================
// Error steps (bijstand and general)
// =============================================================================

#[then(regex = r#"^the execution fails with "([^"]+)"$"#)]
fn assert_execution_fails_with(world: &mut RegelrechtWorld, expected_message: String) {
    assert!(
        world.error.is_some(),
        "Expected execution to fail, but it succeeded with result: {:?}",
        world.result
    );

    let error_msg = world.error_message().unwrap_or_default();

    // Normalize expected message for cross-engine compatibility
    // Both Python and Rust engine now use "No regulation found for mandatory delegation"
    let normalized_expected = expected_message.to_lowercase();

    assert!(
        error_msg.to_lowercase().contains(&normalized_expected),
        "Expected error to contain '{}', got: '{}'",
        expected_message,
        error_msg
    );
}

// =============================================================================
// Erfgrensbeplanting steps
// =============================================================================

#[then(regex = r#"^the minimale_afstand_cm is "(\d+)"$"#)]
fn assert_minimale_afstand_cm(world: &mut RegelrechtWorld, expected: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let expected_cm: i64 = expected
        .parse()
        .unwrap_or_else(|_| panic!("Invalid cm value: {}", expected));

    let actual = world.get_output("minimale_afstand_cm");
    match actual {
        Some(Value::Int(n)) => {
            assert_eq!(
                *n, expected_cm,
                "Expected minimale_afstand_cm to be {}, got {}",
                expected_cm, n
            );
        }
        Some(Value::Float(f)) => {
            let actual_int = f.round() as i64;
            assert_eq!(
                actual_int, expected_cm,
                "Expected minimale_afstand_cm to be {}, got {} (rounded from {})",
                expected_cm, actual_int, f
            );
        }
        _ => panic!(
            "Expected minimale_afstand_cm to be a number, got {:?}",
            actual
        ),
    }
}

#[then(regex = r#"^the minimale_afstand_m is "([0-9.]+)"$"#)]
fn assert_minimale_afstand_m(world: &mut RegelrechtWorld, expected: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let expected_m: f64 = expected
        .parse()
        .unwrap_or_else(|_| panic!("Invalid meter value: {}", expected));

    let actual = world.get_output("minimale_afstand_m");
    match actual {
        Some(Value::Float(f)) => {
            let diff = (f - expected_m).abs();
            assert!(
                diff < 0.001,
                "Expected minimale_afstand_m to be {}, got {} (diff: {})",
                expected_m,
                f,
                diff
            );
        }
        Some(Value::Int(n)) => {
            let actual_f = *n as f64;
            let diff = (actual_f - expected_m).abs();
            assert!(
                diff < 0.001,
                "Expected minimale_afstand_m to be {}, got {} (diff: {})",
                expected_m,
                actual_f,
                diff
            );
        }
        _ => panic!(
            "Expected minimale_afstand_m to be a number, got {:?}",
            actual
        ),
    }
}

// =============================================================================
// Zorgtoeslag steps
// =============================================================================

#[then("the citizen has the right to healthcare allowance")]
fn assert_has_right_to_healthcare_allowance(world: &mut RegelrechtWorld) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let has_right = world.get_output("heeft_recht_op_zorgtoeslag");
    assert!(
        matches!(has_right, Some(Value::Bool(true))),
        "Expected heeft_recht_op_zorgtoeslag to be true, got {:?}",
        has_right
    );
}

#[then("the citizen does not have the right to healthcare allowance")]
fn assert_no_right_to_healthcare_allowance(world: &mut RegelrechtWorld) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let has_right = world.get_output("heeft_recht_op_zorgtoeslag");
    assert!(
        matches!(has_right, Some(Value::Bool(false))),
        "Expected heeft_recht_op_zorgtoeslag to be false, got {:?}",
        has_right
    );
}

#[then(regex = r#"^the standard premium is "(\d+)" eurocent$"#)]
fn assert_standard_premium_eurocent(world: &mut RegelrechtWorld, expected: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let expected_amount: i64 = expected
        .parse()
        .unwrap_or_else(|_| panic!("Invalid eurocent value: {}", expected));

    let actual = world.get_output("standaardpremie");
    match actual {
        Some(Value::Int(n)) => {
            assert_eq!(
                *n, expected_amount,
                "Expected standaardpremie to be {} eurocent, got {}",
                expected_amount, n
            );
        }
        Some(Value::Float(f)) => {
            let actual_int = f.round() as i64;
            assert_eq!(
                actual_int, expected_amount,
                "Expected standaardpremie to be {} eurocent, got {} (rounded from {})",
                expected_amount, actual_int, f
            );
        }
        _ => panic!("Expected standaardpremie to be a number, got {:?}", actual),
    }
}

#[then(regex = r#"^the allowance amount is "([0-9.]+)" euro$"#)]
fn assert_allowance_amount_euro(world: &mut RegelrechtWorld, expected: String) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );

    let expected_euro: f64 = expected
        .parse()
        .unwrap_or_else(|_| panic!("Invalid euro value: {}", expected));

    let actual = world.get_output("hoogte_zorgtoeslag");
    match actual {
        Some(Value::Float(f)) => {
            // Convert eurocent to euro for comparison
            let actual_euro = f / 100.0;
            let diff = (actual_euro - expected_euro).abs();
            assert!(
                diff < 0.01,
                "Expected zorgtoeslag_bedrag to be {} euro, got {} euro (diff: {})",
                expected_euro,
                actual_euro,
                diff
            );
        }
        Some(Value::Int(n)) => {
            // Convert eurocent to euro
            let actual_euro = *n as f64 / 100.0;
            let diff = (actual_euro - expected_euro).abs();
            assert!(
                diff < 0.01,
                "Expected zorgtoeslag_bedrag to be {} euro, got {} euro (diff: {})",
                expected_euro,
                actual_euro,
                diff
            );
        }
        _ => panic!(
            "Expected zorgtoeslag_bedrag to be a number, got {:?}",
            actual
        ),
    }
}
