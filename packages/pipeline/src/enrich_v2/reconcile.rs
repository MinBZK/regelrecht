//! The closing pass: connect what exists by now, and nothing else.
//!
//! Enrichment walks a law in windows. Article 1 is visited while article 5 has
//! no model, so a binding from 1 to an output of 5 cannot be laid there: that
//! output does not exist yet. Only the last window sees everything before it.
//! Part of what survives the gates is therefore not a defect but a measurement
//! taken too early.
//!
//! Most of that class is removed before it arises, by never cutting a
//! top-level article in half at a window boundary (see [`super::refgraph`] and
//! `plan_chunk`). What is left is what this pass is for, and it is deliberately
//! narrow. It runs over a law that has already been through every gate, so
//! anything it adds is a deterioration. It therefore:
//!
//! - **lays a binding only when the name already matches.** An input with no
//!   `source` whose name is exactly an output another entry of this same law
//!   declares is a binding nobody has to think about.
//! - **never interprets.** No article is remodelled, no logic is touched, no
//!   marking about the content is added or taken away.
//! - **refuses on any doubt.** Two entries declaring the same output, a
//!   producer parameter this entry cannot supply, an input entry whose YAML
//!   does not start with `- name:` — all of those are left alone and handed to
//!   the agent as a lead instead.
//!
//! ## How much of it is mechanical
//!
//! Measured on the round-4 corpus, at the window size that runs today, eight
//! bindings fall outside their window after the cohesion rule. Five of those
//! eight have an input name identical to the output name — the mechanical
//! share. The other three name the same concept with different words, which is
//! the agent's part and the reason the lead channel exists.
//!
//! ## What keeps it from breaking anything
//!
//! [`apply`] edits the text rather than round-tripping the document, so the
//! diff is the inserted `source:` blocks and nothing else, and the caller
//! counts findings before and after. A pass whose findings did not fall is
//! reverted by the caller — the count is the guard, not the intent.

use std::collections::BTreeSet;

use serde_yaml_ng::Value;

use super::refgraph::Graph;

/// One binding the closing pass can lay without deciding anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Entry that reads the value.
    pub consumer: String,
    /// Name of the input entry, which is also the output name.
    pub name: String,
    /// Entry that produces it.
    pub producer: String,
    /// Producer parameters, each resolved to a `$name` the consumer has.
    pub parameters: Vec<(String, String)>,
}

impl Link {
    /// One line for a log or a report.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "art. {}: \"{}\" gebonden aan art. {}",
            self.consumer, self.name, self.producer
        )
    }
}

/// Something the closing pass will not do on its own, phrased for the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lead {
    /// Entry the lead is about.
    pub article: String,
    /// What the file says and what now exists beside it.
    pub detail: String,
}

impl Lead {
    /// One line for a log or a prompt.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("[reconcile] art. {}: {}", self.article, self.detail)
    }
}

/// What the closing pass found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Bindings whose names already match, ready to be written.
    pub links: Vec<Link>,
    /// Everything else worth a look, for the agent.
    pub leads: Vec<Lead>,
}

impl Plan {
    /// Nothing to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty() && self.leads.is_empty()
    }
}

/// Read one law and decide what can be connected.
#[must_use]
pub fn plan(doc: &Value) -> Plan {
    let graph = Graph::scan(doc);
    let mut plan = Plan::default();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();

    for article in &articles {
        let Some(number) = article.get("number").and_then(Value::as_str) else {
            continue;
        };
        let Some(execution) = article
            .get("machine_readable")
            .and_then(|mr| mr.get("execution"))
        else {
            continue;
        };
        let in_scope = names_in_scope(execution);

        for input in execution
            .get("input")
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            let Some(name) = input.get("name").and_then(Value::as_str) else {
                continue;
            };
            // An input that already says where it comes from is not this
            // pass's business, whatever it says. Rewiring a source that names
            // another law is a statement about which law owns the concept, and
            // that is interpretation.
            if !is_unbound(input) {
                continue;
            }
            let Some(producers) = graph.producers.get(name) else {
                continue;
            };
            let elsewhere: Vec<_> = producers.iter().filter(|p| p.entry != number).collect();
            match elsewhere.as_slice() {
                [] => {}
                [producer] => match resolve_parameters(&producer.parameters, &in_scope) {
                    Some(parameters) => plan.links.push(Link {
                        consumer: number.to_string(),
                        name: name.to_string(),
                        producer: producer.entry.clone(),
                        parameters,
                    }),
                    None => plan.leads.push(Lead {
                        article: number.to_string(),
                        detail: format!(
                            "invoer \"{name}\" is ongebonden en art. {} levert die output, maar \
                             deze entry kan niet elke parameter aanleveren die art. {} vraagt \
                             ({}). Bind hem met de juiste parameters of laat hem staan",
                            producer.entry,
                            producer.entry,
                            producer.parameters.join(", ")
                        ),
                    }),
                },
                many => plan.leads.push(Lead {
                    article: number.to_string(),
                    detail: format!(
                        "invoer \"{name}\" is ongebonden en meer dan één entry levert die output \
                         ({}). Welke bedoeld is, is geen mechanische keuze",
                        many.iter()
                            .map(|p| p.entry.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }),
            }
        }

        // An open term is a value the law leaves to somebody else. When
        // another entry of this same law now declares that very name as an
        // output, one of the two is wrong, and which one is a legal question.
        // The pass says so and touches neither.
        for open_term in article
            .get("machine_readable")
            .and_then(|mr| mr.get("open_terms"))
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            let Some(id) = open_term.get("id").and_then(Value::as_str) else {
                continue;
            };
            let elsewhere: Vec<&str> = graph
                .producers
                .get(id)
                .into_iter()
                .flatten()
                .filter(|p| p.entry != number)
                .map(|p| p.entry.as_str())
                .collect();
            if elsewhere.is_empty() {
                continue;
            }
            plan.leads.push(Lead {
                article: number.to_string(),
                detail: format!(
                    "open term \"{id}\" staat hier, en art. {} levert inmiddels een output met \
                     diezelfde naam. Eén van de twee is de bron; lees de invoer uit die entry of \
                     laat de open term staan met de reden erbij",
                    elsewhere.join(", ")
                ),
            });
        }
    }
    plan
}

/// Whether an input says nothing about where its value comes from.
///
/// A missing `source`, or one that is an empty mapping. A `source` carrying a
/// `description` is a motivated external fact and stays untouched.
fn is_unbound(input: &Value) -> bool {
    match input.get("source") {
        None => true,
        Some(Value::Null) => true,
        Some(Value::Mapping(map)) => map.is_empty(),
        Some(_) => false,
    }
}

/// Every name this entry can pass on: its own parameters and its own inputs.
fn names_in_scope(execution: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for key in ["parameters", "input"] {
        for item in execution
            .get(key)
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            if let Some(name) = item.get("name").and_then(Value::as_str) {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// Map the producer's parameters onto names the consumer has. All or nothing:
/// a producer parameter the consumer cannot supply makes the whole binding a
/// lead, because guessing which of its values to pass is interpretation.
fn resolve_parameters(
    wanted: &[String],
    in_scope: &BTreeSet<String>,
) -> Option<Vec<(String, String)>> {
    wanted
        .iter()
        .map(|p| in_scope.contains(p).then(|| (p.clone(), format!("${p}"))))
        .collect()
}

/// Write the planned bindings into the law text.
///
/// A textual splice rather than a document round-trip: the corpus files carry
/// hand-wrapped block scalars that a re-serialisation would reflow, and a
/// closing pass whose diff is the whole file cannot be reviewed for what it
/// actually did. The `source:` block is inserted directly under the input's
/// `- name:` line, which is where a mapping key may go and where it is
/// findable again.
///
/// Returns the new text and the links actually written. A link whose entry
/// does not start with `- name:` is dropped rather than guessed at.
#[must_use]
pub fn apply(yaml: &str, links: &[Link]) -> (String, Vec<Link>) {
    let mut lines: Vec<String> = yaml.lines().map(str::to_string).collect();
    let mut written = Vec::new();
    // Back to front, so an insertion never moves a line number still to come.
    let mut planned: Vec<(usize, &Link, String)> = Vec::new();
    for link in links {
        if let Some((line, indent)) = find_input_line(&lines, &link.consumer, &link.name) {
            planned.push((line, link, indent));
        }
    }
    planned.sort_by_key(|(line, _, _)| std::cmp::Reverse(*line));
    for (line, link, indent) in planned {
        let mut block = vec![format!("{indent}source:")];
        block.push(format!("{indent}  output: {}", link.name));
        if !link.parameters.is_empty() {
            block.push(format!("{indent}  parameters:"));
            for (key, value) in &link.parameters {
                block.push(format!("{indent}    {key}: {value}"));
            }
        }
        lines.splice(line + 1..line + 1, block);
        written.push(link.clone());
    }
    written.reverse();
    let mut out = lines.join("\n");
    if yaml.ends_with('\n') {
        out.push('\n');
    }
    (out, written)
}

/// The line holding `- name: <name>` inside the `input:` block of one entry,
/// and the indent its sibling keys sit at.
///
/// Scoped three times over — to the consuming entry, to its `input:` key, to
/// the item that opens with this name — because the same name occurs again as
/// the producer's `output`, and a splice into that one would be silent and
/// wrong.
///
/// Deliberately strict. The item has to open with `- name:`, because that is
/// the only line whose position is certain without re-parsing, and every
/// generated corpus file writes it that way. Anything else is left to the
/// agent.
fn find_input_line(lines: &[String], consumer: &str, name: &str) -> Option<(usize, String)> {
    let (start, end) = entry_span(lines, consumer)?;
    let (block_start, block_end) = key_span(lines, start, end, "input")?;
    let mut found: Option<(usize, String)> = None;
    for (number, line) in lines.iter().enumerate().take(block_end).skip(block_start) {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("- name: ") else {
            continue;
        };
        if rest.trim().trim_matches(['\'', '"']) != name {
            continue;
        }
        if found.is_some() {
            return None;
        }
        let dash = line.len() - trimmed.len();
        found = Some((number, " ".repeat(dash + 2)));
    }
    found
}

/// Line range of the article entry with this `number`, from its `- number:`
/// line up to the next sequence item at the same indent.
fn entry_span(lines: &[String], number: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| {
        line.trim_start()
            .strip_prefix("- number: ")
            .is_some_and(|rest| rest.trim().trim_matches(['\'', '"']) == number)
    })?;
    let indent = lines[start].len() - lines[start].trim_start().len();
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.is_empty()
                && line.len() - trimmed.len() <= indent
                && (trimmed.starts_with("- ") || line.len() - trimmed.len() < indent)
        })
        .map_or(lines.len(), |(i, _)| i);
    Some((start, end))
}

/// Line range of the block under `key`, searched within `[start, end)`: the
/// lines after the `key:` line that are indented deeper than it.
fn key_span(lines: &[String], start: usize, end: usize, key: &str) -> Option<(usize, usize)> {
    let needle = format!("{key}:");
    let at = (start..end).find(|i| lines[*i].trim() == needle)?;
    let indent = lines[at].len() - lines[at].trim_start().len();
    let block_end = (at + 1..end)
        .find(|i| {
            let trimmed = lines[*i].trim_start();
            !trimmed.is_empty() && lines[*i].len() - trimmed.len() <= indent
        })
        .unwrap_or(end);
    Some((at + 1, block_end))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A law whose first entry reads a value its third entry produces: the
    /// window that saw entry 1 could not have known the name existed.
    const TOO_EARLY: &str = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De premie, bedoeld in artikel 2, telt mee.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        input:
          - name: standaardpremie
            type: amount
        output:
          - name: hoogte
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        output:
          - name: standaardpremie
";

    fn doc(yaml: &str) -> Value {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    #[test]
    fn an_unbound_input_that_another_entry_produces_is_mechanical() {
        let plan = plan(&doc(TOO_EARLY));
        assert_eq!(plan.leads, vec![]);
        assert_eq!(
            plan.links,
            vec![Link {
                consumer: "1".to_string(),
                name: "standaardpremie".to_string(),
                producer: "2".to_string(),
                parameters: vec![("bsn".to_string(), "$bsn".to_string())],
            }]
        );
    }

    #[test]
    fn applying_the_plan_only_inserts_the_source_block() {
        let plan = plan(&doc(TOO_EARLY));
        let (out, written) = apply(TOO_EARLY, &plan.links);
        assert_eq!(written.len(), 1);
        assert!(out.contains(
            "          - name: standaardpremie\n            source:\n              output: \
             standaardpremie\n              parameters:\n                bsn: $bsn\n"
        ));
        // Everything else stands: same line count plus the four inserted.
        assert_eq!(out.lines().count(), TOO_EARLY.lines().count() + 4);
        // And the result is still the same document plus that binding.
        let after: Value = serde_yaml_ng::from_str(&out).unwrap();
        let source = &after["articles"][0]["machine_readable"]["execution"]["input"][0]["source"];
        assert_eq!(source["output"].as_str(), Some("standaardpremie"));
        assert_eq!(source["parameters"]["bsn"].as_str(), Some("$bsn"));
    }

    #[test]
    fn applying_an_empty_plan_leaves_the_bytes_alone() {
        let (out, written) = apply(TOO_EARLY, &[]);
        assert_eq!(out, TOO_EARLY);
        assert!(written.is_empty());
    }

    #[test]
    fn a_bound_input_is_never_rewired() {
        let law = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De premie, bedoeld in artikel 2, telt mee.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        input:
          - name: standaardpremie
            source:
              regulation: andere_wet
              output: standaardpremie
        output:
          - name: hoogte
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    machine_readable:
      execution:
        output:
          - name: standaardpremie
";
        assert!(plan(&doc(law)).is_empty());
    }

    #[test]
    fn an_input_whose_producer_wants_a_parameter_this_entry_lacks_is_a_lead() {
        let law = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De premie, bedoeld in artikel 2, telt mee.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        input:
          - name: standaardpremie
        output:
          - name: hoogte
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    machine_readable:
      execution:
        parameters:
          - name: berekeningsjaar
        output:
          - name: standaardpremie
";
        let plan = plan(&doc(law));
        assert!(plan.links.is_empty());
        assert_eq!(plan.leads.len(), 1);
        assert!(plan.leads[0].detail.contains("berekeningsjaar"));
    }

    #[test]
    fn two_entries_producing_the_same_name_is_a_lead_and_not_a_guess() {
        let law = format!(
            "{TOO_EARLY}{}",
            r"  - number: '3'
    text: Ook hier staat een standaardpremie.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        output:
          - name: standaardpremie
"
        );
        let plan = plan(&doc(&law));
        assert!(plan.links.is_empty());
        assert_eq!(plan.leads.len(), 1);
        assert!(plan.leads[0].detail.contains("2, 3"));
    }

    #[test]
    fn an_open_term_another_entry_now_produces_is_reported_and_not_removed() {
        let law = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De premie, bedoeld in artikel 2, telt mee.
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
      execution:
        parameters:
          - name: bsn
        output:
          - name: hoogte
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    machine_readable:
      execution:
        output:
          - name: standaardpremie
";
        let plan = plan(&doc(law));
        assert_eq!(plan.leads.len(), 1);
        assert!(plan.leads[0].detail.contains("open term"));
        // The pass reports it; removing the open term stays a legal decision.
        let (out, _) = apply(law, &plan.links);
        assert!(out.contains("- id: standaardpremie"));
    }

    #[test]
    fn a_self_binding_is_never_planned() {
        let law = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Niets bijzonders.
    machine_readable:
      execution:
        input:
          - name: hoogte
        output:
          - name: hoogte
";
        assert!(plan(&doc(law)).is_empty());
    }

    #[test]
    fn a_producer_without_parameters_gets_a_source_without_parameters() {
        let law = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De premie, bedoeld in artikel 2, telt mee.
    machine_readable:
      execution:
        input:
          - name: standaardpremie
        output:
          - name: hoogte
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    machine_readable:
      execution:
        output:
          - name: standaardpremie
";
        let plan = plan(&doc(law));
        assert_eq!(plan.links.len(), 1);
        assert!(plan.links[0].parameters.is_empty());
        let (out, _) = apply(law, &plan.links);
        let after: Value = serde_yaml_ng::from_str(&out).unwrap();
        let source = &after["articles"][0]["machine_readable"]["execution"]["input"][0]["source"];
        assert_eq!(source["output"].as_str(), Some("standaardpremie"));
        assert!(source.get("parameters").is_none());
    }
}
