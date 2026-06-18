// Emitter: grammar -> Rust cucumber step fns. include!'d by build.rs.
// Relies on grammar_model.rs being include!'d first.
fn emit_rust(grammar: &Grammar) -> String {
    let mut s = String::new();
    s.push_str("// @generated from bdd/grammar.yaml — do not edit.\n");
    s.push_str("use cucumber::{given, when, then};\n\n");
    for (i, step) in grammar.steps.iter().enumerate() {
        let fn_name = format!("step_{}_{}", i, step.id);
        let macro_name = step.keyword.as_str(); // given|when|then
        let attr = if needs_regex(step) {
            // Emit a normal (escaped) string literal: a raw string can't hold the
            // `\d` / `\"` the regex needs. Escape backslashes first, then quotes.
            let escaped = to_regex(step).replace('\\', "\\\\").replace('"', "\\\"");
            format!("regex = \"{escaped}\"")
        } else {
            format!("expr = \"{}\"", to_cucumber_expr(step).replace('"', "\\\""))
        };

        // Build the function parameter list: cucumber passes captures as fn
        // params (String for {string} and regex string captures; we parse
        // numbers from String ourselves to keep one code path).
        let mut params = String::new();
        let mut pushes = String::new();
        for (n, a) in step.args.iter().enumerate() {
            params.push_str(&format!(", arg{n}: String"));
            match a.ty.as_str() {
                "number" => pushes.push_str(&format!(
                    "    args.push(ArgValue::Num(arg{n}.parse::<f64>().expect(\"numeric arg\")));\n"
                )),
                _ => pushes.push_str(&format!("    args.push(ArgValue::Str(arg{n}));\n")),
            }
        }
        // literals appended after captured args
        let mut lit_pushes = String::new();
        for lit in &step.literals {
            match lit {
                serde_yaml_ng::Value::Bool(b) => {
                    lit_pushes.push_str(&format!("    args.push(ArgValue::Bool({b}));\n"))
                }
                serde_yaml_ng::Value::Number(num) => lit_pushes.push_str(&format!(
                    "    args.push(ArgValue::Num({}f64));\n",
                    num.as_f64().unwrap()
                )),
                serde_yaml_ng::Value::String(text) => lit_pushes.push_str(&format!(
                    "    args.push(ArgValue::Str({:?}.to_string()));\n",
                    text
                )),
                _ => {}
            }
        }

        let step_param = if step.datatable {
            ", step: &cucumber::gherkin::Step"
        } else {
            ""
        };
        let table_expr = if step.datatable {
            "step.table.as_ref().map(|t| t.rows.clone())"
        } else {
            "None"
        };

        s.push_str(&format!("#[{macro_name}({attr})]\n"));
        s.push_str(&format!(
            "async fn {fn_name}(world: &mut RegelrechtWorld{params}{step_param}) {{\n"
        ));
        s.push_str("    let mut args: Vec<ArgValue> = Vec::new();\n");
        s.push_str(&pushes);
        s.push_str(&lit_pushes);
        s.push_str(&format!("    let table = {table_expr};\n"));
        s.push_str(&format!(
            "    world.dispatch(\"{}\", args, table).await;\n",
            step.action
        ));
        s.push_str("}\n\n");
    }
    s
}
