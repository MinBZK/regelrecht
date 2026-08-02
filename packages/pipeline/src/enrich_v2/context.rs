//! What the worker puts in front of the agent before it translates a window.
//!
//! The prompt used to hand over the law file and the sentence "process ONLY
//! these articles". Everything else the agent had to find for itself, and
//! whether it looked was not observable. Two of the three things it needed are
//! not findable by reading the window at all.
//!
//! The first is where an article sits. A definition at the head of an afdeling
//! governs that afdeling without any article referring to it, so which
//! provisions bear on article 8 follows from the containers around article 8.
//! That is what `placement` is for, and [`super::assemble`] already carries it
//! per article as `path`.
//!
//! The second is what modifies the window from outside it. Article 10 may say
//! "in afwijking van artikel 8", and then the influence runs from 10 to 8 while
//! the reference points the other way. An agent given only article 8 translates
//! a rule that the law does not have. Finding those means scanning the whole
//! law, which is cheap because the text is already here, and it is why this
//! module resolves inbound references while cross-law resolution stays narrow.
//!
//! Not every inbound reference belongs in the brief. "Het bedrag, bedoeld in
//! artikel 8" uses article 8 and leaves it alone. The connectives that mark a
//! modifying reference are the ones the coverage check already knows, which is
//! not a coincidence: both are asking where a rule stops being what it looks
//! like.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::assemble::{AssembledArticle, LawContext};

/// File the worker writes beside the law for the agent to read.
///
/// Markdown rather than YAML because it is read and never parsed, and a dot
/// prefix so a corpus checkout does not gain a stray document.
pub const CONTEXT_BRIEF: &str = ".enrichment-context.md";

/// Words that make an inbound reference modifying rather than merely using.
///
/// A shorter list than the coverage check's, because that one asks whether a
/// model has branches and this one asks whether one article bends another.
/// "Tenzij" and "mits" qualify a rule inside its own article; they say nothing
/// about an article elsewhere.
const MODIFYING: &[&str] = &[
    "in afwijking van",
    "onverminderd",
    "in aanvulling op",
    "niet van toepassing",
    "geldt niet",
    "blijft buiten toepassing",
    "behoudens",
    "met dien verstande",
    "voor de toepassing van",
];

/// One article bending another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    /// The article doing the modifying.
    pub from: String,
    /// The article in the window that it modifies.
    pub to: String,
    /// The connective that made this a modification.
    pub connective: String,
    /// The sentence it appeared in, so the agent can judge for itself.
    pub sentence: String,
}

/// Split a text into sentences, crudely and on purpose.
///
/// A statutory sentence ends at a full stop followed by a space, and the
/// abbreviations that would trip this up ("art.", "jo.", "e.d.") do not change
/// which connective sits next to which article number. Getting a boundary
/// wrong widens or narrows the quoted sentence and never changes the verdict.
fn sentences(text: &str) -> Vec<String> {
    strip_reference_links(text)
        .split_inclusive(['.', ';'])
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Drop the markdown reference-link machinery the harvester leaves in the text.
///
/// Two separate hazards. The footer lines (`[ref1]: https://wetten.nl/...`)
/// contain full stops inside a URL, so sentence splitting cuts through them and
/// the halves end up quoted as statutory text in the brief. And the fragment
/// `#Artikel1` reads as a reference to article 1 to anything scanning for the
/// word, which is how article 1 of the Awir acquired two modifiers it does not
/// have.
fn strip_reference_links(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim_start();
            !(t.starts_with('[') && t.contains("]: ") && t.contains("://"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Article numbers referred to in one sentence.
///
/// Handles "artikel 8", "artikelen 8 en 9", "artikel 8, tweede lid" and the
/// letter-suffixed numbering the corpus uses ("artikel 3a"). The lid is
/// dropped: the node is the article, and the address only decides which
/// fragment is offered, which happens elsewhere.
pub fn referenced_articles(sentence: &str) -> Vec<String> {
    let lower = sentence.to_lowercase();
    let mut out = Vec::new();
    let mut rest = lower.as_str();
    while let Some(pos) = rest.find("artikel") {
        let after = &rest[pos + "artikel".len()..];
        // `#artikel1` in a URL is not a reference. A real one separates the
        // word from the number, with a space or the plural ending.
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            rest = after;
            continue;
        }
        // "artikel 8 van de Zorgverzekeringswet" points outside this law, and
        // this brief is about what one law does to itself. Cross-law edges are
        // the work queue's business, not the brief's.
        if is_external_reference(after) {
            rest = after;
            continue;
        }
        // "artikelen 8 en 9" enumerates; "artikel 8" does not.
        let tail = after.strip_prefix("en").unwrap_or(after);
        let mut scan = tail;
        loop {
            let trimmed = scan.trim_start_matches([' ', ',']);
            let taken: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                .collect();
            let number: String = taken
                .chars()
                .take_while(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                .collect();
            if number.is_empty() || !number.starts_with(|c: char| c.is_ascii_digit()) {
                break;
            }
            out.push(number.clone());
            let consumed = trimmed.len() - trimmed[number.len()..].len();
            scan = &trimmed[consumed..];
            // Only "en" or a comma continues an enumeration.
            let next = scan.trim_start();
            if let Some(after_en) = next.strip_prefix("en ") {
                scan = after_en;
            } else if next.starts_with(", ") && next[2..].starts_with(|c: char| c.is_ascii_digit())
            {
                scan = &next[2..];
            } else {
                break;
            }
        }
        rest = after;
    }
    out.sort();
    out.dedup();
    out
}

/// Whether what follows an article number sends it to another law.
///
/// "van de", "van het" and "van die" introduce the statute the article belongs
/// to. Without this the brief claims that an article of some other law modifies
/// an article here, which is worse than saying nothing: it is a relation the
/// reader cannot check against the file in front of them.
fn is_external_reference(after_word: &str) -> bool {
    let tail: String = after_word
        .chars()
        .skip_while(|c| c.is_whitespace() || c.is_ascii_digit() || *c == ',')
        .take(60)
        .collect();
    let tail = tail.trim_start();
    for lead in [
        "van de ",
        "van het ",
        "van die ",
        "van deze wet",
        "van dat ",
    ] {
        if tail.starts_with(lead) {
            // "van deze wet" points back here, so it is not external.
            return !tail.starts_with("van deze wet");
        }
    }
    // A lid qualifier may sit in between: "artikel 8, tweede lid, van de Awir".
    if let Some(rest) = tail.strip_prefix("tweede lid").or_else(|| {
        ["eerste lid", "derde lid", "vierde lid", "vijfde lid"]
            .iter()
            .find_map(|l| tail.strip_prefix(l))
    }) {
        let rest = rest.trim_start_matches([',', ' ']);
        return rest.starts_with("van de ")
            || rest.starts_with("van het ")
            || rest.starts_with("van die ");
    }
    false
}

/// Articles outside the window that modify articles inside it.
///
/// Scans the whole law: an article that bends the window can sit anywhere, and
/// document order says nothing about it. Self-references are dropped, because
/// an article qualifying its own lid is already in front of the agent.
#[must_use]
pub fn inbound_modifiers(law: &LawContext, window: &[String]) -> Vec<Inbound> {
    let target: std::collections::BTreeSet<&str> = window.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    for article in &law.articles {
        if target.contains(article.number.as_str()) {
            continue;
        }
        for sentence in sentences(&article.text) {
            let lower = sentence.to_lowercase();
            let Some(connective) = MODIFYING.iter().find(|c| lower.contains(**c)) else {
                continue;
            };
            for number in referenced_articles(&sentence) {
                if !target.contains(number.as_str()) {
                    continue;
                }
                out.push(Inbound {
                    from: article.number.clone(),
                    to: number,
                    connective: (*connective).to_owned(),
                    sentence: sentence.clone(),
                });
            }
        }
    }
    out
}

/// Definition articles that govern an article, by container.
///
/// A definition provision applies to whatever encloses it: one in hoofdstuk 1
/// reaches everything below, one at the head of an afdeling reaches that
/// afdeling. The path is a prefix chain, so a definition governs an article
/// when its own path is a prefix of the article's, and a definition with no
/// path governs the whole law.
#[must_use]
pub fn governing_definitions<'a>(
    law: &'a LawContext,
    article: &AssembledArticle,
) -> Vec<&'a AssembledArticle> {
    law.definitions
        .iter()
        .filter(|d| d.path.is_empty() || article.path.starts_with(d.path.as_str()))
        .collect()
}

/// The brief the agent reads before it translates.
///
/// Ordered by what it costs the agent to miss: where the article sits, then
/// what defines its terms, then what bends it. Every section says when it is
/// empty, because a heading with nothing under it reads as "not looked at".
#[must_use]
pub fn render_brief(law: &LawContext, window: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Context for {} ({}, {})\n\n\
         Written by the worker. You may cite anything in this file. Nothing here\n\
         is a source for a rule: the rule comes from the statutory text of the\n\
         article you are translating.\n",
        law.law_id, law.bwb_id, law.valid_from
    );

    let inbound = inbound_modifiers(law, window);
    let by_target: BTreeMap<&str, Vec<&Inbound>> =
        inbound.iter().fold(BTreeMap::new(), |mut acc, i| {
            acc.entry(i.to.as_str()).or_default().push(i);
            acc
        });

    for number in window {
        let Some(article) = law.articles.iter().find(|a| &a.number == number) else {
            continue;
        };
        let _ = writeln!(out, "## Article {number}");
        if article.path.is_empty() {
            let _ = writeln!(out, "\nPlacement: no enclosing container.");
        } else {
            let _ = writeln!(out, "\nPlacement: {}", article.path);
        }

        let definitions = governing_definitions(law, article);
        if definitions.is_empty() {
            let _ = writeln!(out, "\nNo definition provision governs this article.");
        } else {
            let _ = writeln!(out, "\n### Definitions that govern it");
            for d in definitions {
                let scope = if d.path.is_empty() {
                    "whole law".to_owned()
                } else {
                    d.path.clone()
                };
                let _ = writeln!(out, "\n**Article {} ({scope})**\n\n{}", d.number, d.text);
            }
        }

        match by_target.get(number.as_str()) {
            None => {
                let _ = writeln!(
                    out,
                    "\n### What modifies it\n\nNo other article in this law modifies it."
                );
            }
            Some(items) => {
                let _ = writeln!(
                    out,
                    "\n### What modifies it\n\nThese articles bend this one. Translate the rule \
                     as they leave it, not as this article reads alone."
                );
                for i in items {
                    let _ = writeln!(
                        out,
                        "\n- **Article {}** (\"{}\"): {}",
                        i.from, i.connective, i.sentence
                    );
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Assemble the law beside `yaml_abs` and write the brief next to it.
///
/// Returns the path written, or `None` when the file carries no articles.
///
/// The source-context sidecar is optional. Without it the placement lines say
/// so instead of being absent, because "no enclosing container" and "nobody
/// looked" must not read the same. The inbound scan needs no sidecar at all:
/// it reads the article text this file already carries.
pub fn write_brief(
    yaml_abs: &std::path::Path,
    window: Option<&[String]>,
) -> Option<std::path::PathBuf> {
    let body = std::fs::read_to_string(yaml_abs).ok()?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&body).ok()?;

    let dir = yaml_abs.parent()?;
    let sidecar: Option<super::source_gate::ContextSidecar> =
        std::fs::read_to_string(dir.join(super::source_gate::CONTEXT_SIDECAR))
            .ok()
            .and_then(|s| serde_yaml_ng::from_str(&s).ok());

    // Without the gate's list of real article numbers the split of a corpus
    // entry like `5.2.1` is guesswork, so fall back to the entry numbers
    // themselves: coarser, and never wrong about which article exists.
    let known: Vec<String> = match sidecar.as_ref() {
        Some(s) => s.articles.keys().cloned().collect(),
        None => doc
            .get("articles")
            .and_then(serde_yaml_ng::Value::as_sequence)
            .map(|arts| {
                arts.iter()
                    .filter_map(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    };

    let law = super::assemble::assemble(&doc, &known, sidecar.as_ref());
    if law.articles.is_empty() {
        return None;
    }
    // The window arrives in corpus entry numbers (`1.1.c`), while the brief
    // speaks in article numbers (`1`), because the article is the unit of work
    // and the entry is a fragment of one. Mapping the window through the same
    // split the assembler uses is what keeps the two from silently missing
    // each other: an unmapped window matches nothing and yields a brief with
    // a heading and no content.
    let scope: Vec<String> = match window {
        Some(w) => {
            let mut mapped: Vec<String> = w
                .iter()
                .map(|entry| super::assemble::split_number(&known, entry).0.to_owned())
                .filter(|a| !a.is_empty())
                .collect();
            mapped.sort();
            mapped.dedup();
            mapped
        }
        None => law.articles.iter().map(|a| a.number.clone()).collect(),
    };
    let scope = if scope.is_empty() {
        law.articles.iter().map(|a| a.number.clone()).collect()
    } else {
        scope
    };

    let path = dir.join(CONTEXT_BRIEF);
    std::fs::write(&path, render_brief(&law, &scope)).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(number: &str, path: &str, text: &str) -> AssembledArticle {
        AssembledArticle {
            number: number.to_owned(),
            fragments: vec![number.to_owned()],
            text: text.to_owned(),
            path: path.to_owned(),
            has_model: false,
        }
    }

    fn law(articles: Vec<AssembledArticle>, definitions: Vec<AssembledArticle>) -> LawContext {
        LawContext {
            law_id: "test_wet".to_owned(),
            bwb_id: "BWBR0000000".to_owned(),
            valid_from: "2025-01-01".to_owned(),
            definitions,
            articles,
        }
    }

    #[test]
    fn finds_a_plain_reference() {
        assert_eq!(referenced_articles("bedoeld in artikel 8"), vec!["8"]);
    }

    #[test]
    fn finds_an_enumeration() {
        assert_eq!(
            referenced_articles("de artikelen 8 en 9 zijn van toepassing"),
            vec!["8", "9"]
        );
    }

    #[test]
    fn keeps_the_article_and_drops_the_lid() {
        // The node is the article; the lid is an address decided elsewhere.
        assert_eq!(
            referenced_articles("in afwijking van artikel 8, tweede lid"),
            vec!["8"]
        );
    }

    #[test]
    fn handles_letter_suffixed_numbering() {
        assert_eq!(referenced_articles("artikel 3a bepaalt"), vec!["3a"]);
    }

    #[test]
    fn modifying_reference_from_outside_the_window_is_found() {
        // The failure this module exists for: the influence runs from 10 to 8
        // while the reference points from 10 at 8.
        let l = law(
            vec![
                article("8", "", "De toeslag bedraagt het standaardbedrag."),
                article(
                    "10",
                    "",
                    "In afwijking van artikel 8 bedraagt de toeslag nihil bij een vermogen boven de grens.",
                ),
            ],
            vec![],
        );
        let found = inbound_modifiers(&l, &["8".to_owned()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].from, "10");
        assert_eq!(found[0].to, "8");
        assert_eq!(found[0].connective, "in afwijking van");
    }

    #[test]
    fn merely_using_an_article_is_not_a_modification() {
        // "Het bedrag, bedoeld in artikel 8" uses article 8 and leaves it be.
        let l = law(
            vec![
                article("8", "", "De toeslag bedraagt het standaardbedrag."),
                article(
                    "12",
                    "",
                    "Het bedrag, bedoeld in artikel 8, wordt jaarlijks bekendgemaakt.",
                ),
            ],
            vec![],
        );
        assert!(inbound_modifiers(&l, &["8".to_owned()]).is_empty());
    }

    #[test]
    fn an_article_does_not_modify_itself() {
        let l = law(
            vec![article(
                "8",
                "",
                "In afwijking van artikel 8, eerste lid, geldt het tweede lid.",
            )],
            vec![],
        );
        assert!(inbound_modifiers(&l, &["8".to_owned()]).is_empty());
    }

    #[test]
    fn a_url_fragment_is_not_a_reference() {
        // Measured on the Awir in round 3: the harvested text carries markdown
        // reference footers, and `#Artikel1` inside a URL made article 1 look
        // like the target of two modifications it does not have.
        assert!(referenced_articles("zie https://wetten.nl/BWBR0008659#Artikel1").is_empty());
    }

    #[test]
    fn reference_footers_are_not_statutory_text() {
        // The footer holds full stops inside a URL, so splitting on sentences
        // cuts through it and the halves get quoted as if they were the law.
        let text = "In afwijking van artikel 8 geldt nihil.\n\n[ref1]: https://wetten.nl/BWBR0008659#Artikel1\n";
        let found = sentences(text);
        assert!(
            found.iter().all(|s| !s.contains("wetten.nl")),
            "got: {found:?}"
        );
    }

    #[test]
    fn a_reference_into_another_law_is_not_an_inbound_modifier() {
        // The brief is about what one law does to itself. A relation to some
        // other statute cannot be checked against the file in front of the
        // reader, and cross-law edges belong to the work queue.
        let l = law(
            vec![
                article("1", "", "Begripsbepalingen."),
                article(
                    "31ter",
                    "",
                    "In afwijking van artikel 31bis geldt dit voor wie recht heeft op een tegemoetkoming als bedoeld in artikel 1 van die wet.",
                ),
            ],
            vec![],
        );
        assert!(
            inbound_modifiers(&l, &["1".to_owned()]).is_empty(),
            "article 1 of another law was read as article 1 of this one"
        );
    }

    #[test]
    fn a_lid_qualifier_does_not_hide_the_other_law() {
        assert!(is_external_reference(
            " 8, tweede lid, van de Zorgverzekeringswet"
        ));
        assert!(is_external_reference(" 8 van de Awir"));
        assert!(!is_external_reference(" 8 geldt niet"));
        assert!(!is_external_reference(" 8 van deze wet"));
    }

    #[test]
    fn definitions_are_scoped_by_container() {
        let l = law(
            vec![article(
                "8",
                "Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht",
                "tekst",
            )],
            vec![
                article("1", "", "In deze wet wordt verstaan onder: a. toeslag: ..."),
                article(
                    "7",
                    "Hoofdstuk 3 Toeslagen",
                    "In dit hoofdstuk wordt verstaan onder: b. peildatum: ...",
                ),
                article(
                    "20",
                    "Hoofdstuk 4 Bezwaar",
                    "In dit hoofdstuk wordt verstaan onder: c. termijn: ...",
                ),
            ],
        );
        let governing: Vec<&str> = governing_definitions(&l, &l.articles[0])
            .iter()
            .map(|d| d.number.as_str())
            .collect();
        // The law-wide one and the enclosing chapter's, never the sibling
        // chapter's.
        assert_eq!(governing, vec!["1", "7"]);
    }

    #[test]
    fn brief_names_an_empty_section_rather_than_omitting_it() {
        // A missing heading reads as "not looked at", which is the ambiguity
        // this whole flow is trying to remove.
        let l = law(vec![article("8", "", "De toeslag bedraagt nihil.")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()]);
        assert!(brief.contains("No definition provision governs this article."));
        assert!(brief.contains("No other article in this law modifies it."));
    }

    #[test]
    fn window_in_entry_numbers_still_finds_its_articles() {
        // The corpus splits below article level, so a window is a list of
        // entries like `1.1.c` while the brief speaks in article numbers. When
        // those two miss each other the brief comes out as a heading with
        // nothing under it, and nothing fails: exactly the silence this flow
        // is built to remove.
        let dir = tempfile::tempdir().expect("tempdir");
        let law = dir.path().join("2026-01-01.yaml");
        std::fs::write(
            &law,
            r#"$schema: https://regelrecht.rijks.app/schema/v0.6.0/schema.json
articles:
  - number: "1.1.a"
    text: "In deze wet wordt verstaan onder: a. toeslag: een tegemoetkoming."
  - number: "8.1"
    text: "Het toetsingsinkomen is het verzamelinkomen."
  - number: "10.1"
    text: "In afwijking van artikel 8 blijft het inkomen buiten beschouwing."
"#,
        )
        .expect("write law");

        // The source gate runs first and leaves the real article numbers
        // behind; without it the split of `8.1` is guesswork.
        std::fs::write(
            dir.path().join(super::super::source_gate::CONTEXT_SIDECAR),
            r#"bwb_id: BWBR0000000
valid_from: "2026-01-01"
articles:
  "1":
    verdict: verified
  "8":
    path: "Hoofdstuk 2 Toeslag"
    verdict: verified
  "10":
    path: "Hoofdstuk 2 Toeslag"
    verdict: verified
"#,
        )
        .expect("write sidecar");

        let written = write_brief(&law, Some(&["8.1".to_owned()])).expect("brief written");
        let brief = std::fs::read_to_string(written).expect("read brief");

        assert!(brief.contains("## Article 8"), "brief was: {brief}");
        assert!(
            brief.contains("**Article 10**"),
            "inbound modifier missing from:\n{brief}"
        );
    }

    #[test]
    fn brief_says_it_is_not_a_source_for_rules() {
        let l = law(vec![article("8", "", "tekst")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()]);
        assert!(brief.contains("Nothing here"));
        assert!(brief.contains("statutory text"));
    }

    #[test]
    fn brief_carries_placement_and_the_modifier() {
        let l = law(
            vec![
                article(
                    "8",
                    "Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht",
                    "De toeslag bedraagt X.",
                ),
                article(
                    "10",
                    "Hoofdstuk 3 Toeslagen",
                    "In afwijking van artikel 8 geldt nihil.",
                ),
            ],
            vec![],
        );
        let brief = render_brief(&l, &["8".to_owned()]);
        assert!(brief.contains("Placement: Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht"));
        assert!(brief.contains("**Article 10**"));
        assert!(brief.contains("in afwijking van"));
    }
}
