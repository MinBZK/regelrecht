//! Which laws and articles one enrichment order pulls in, and where it stops.
//!
//! `--depth` counts **wetssprongen** and not artikelen. Concentric circles, and
//! the circle is the law:
//!
//! - We stand in law A at the named article. Depth 0.
//! - A step inside A costs nothing; the hot path there is followed freely.
//! - A step to law B costs a point: depth 1. Steps inside B are free again.
//! - B to C costs a point: depth 2.
//! - A straight to C also costs one point: depth 1. The depth is the distance
//!   to the law you started in and not the length of the path that found it,
//!   which makes this a breadth-first shortest distance over laws.
//!
//! Inside a law the whole law is *not* enriched. Only what lies on the hot path
//! of the article you came in through is taken. The depth bounds the number of
//! law jumps; the hot path bounds the set of articles within one law.
//!
//! ## De meting die de standaard bepaalt
//!
//! Gemeten met deze code op het corpus, vanaf `--article 69` van de
//! Zorgverzekeringswet, met de stopregels hieronder aan. De meting is gedaan
//! toen de Awb en de Awir nog als "kaderwet" naast de diepte stonden; zonder
//! die uitzondering, die inmiddels geschrapt is, komt diepte 3 op 5 396 in
//! plaats van 5 274 artikelen uit, ruim twee procent meer:
//!
//! | diepte | wetten | artikelen | entries |
//! |-------:|-------:|----------:|--------:|
//! | 0      |      1 |        53 |     434 |
//! | 1      |     21 |       624 |   3 766 |
//! | 2      |     99 |     2 694 |   6 671 |
//! | 3      |    233 |     5 274 |  12 612 |
//!
//! Vanaf de zeven artikelen die de Wet op de zorgtoeslag in de Zvw aanhaalt (1,
//! 18d, 18e, 19, 24, 68b en 69) loopt het iets hoger: 55, 634, 3 369 en 6 061
//! artikelen. De ingang maakt op diepte 0 en 1 nauwelijks verschil, want de
//! vrije stap binnen de wet trekt de wet toch grotendeels mee.
//!
//! Drie dingen volgen daaruit.
//!
//! **De vrije stap binnen de wet is de uitwaaiering, niet de wetssprong.**
//! Diepte 0 is al 53 van de 86 hoofdartikelen van de Zvw, en één wetssprong
//! maakt daar 624 artikelen van. Artikel 69 haalt via de wanbetalersbepalingen
//! het Wetboek van Burgerlijke Rechtsvordering, de Faillissementswet en Boek 2
//! BW binnen, en dat heeft met zorgtoeslag niets te maken.
//!
//! **De standaard is diepte 1.** Diepte 2 is bijna drieduizend artikelen en
//! diepte 3 ruim vijfduizend, verdeeld over 233 wetten: ruim vijf procent van
//! alle wetten in het corpus. Een vangnet dat je per ongeluk aanzet en dat dan
//! de halve wetgeving verrijkt is geen vangnet.
//!
//! **Boven een grens weigert de planner.** [`Plan::refuse_above`] laat het
//! aantal artikelen zien en stopt, in plaats van een run te beginnen die dagen
//! duurt. `enrich-once` zet die grens op 200, dus zelfs diepte 1 op deze wet
//! moet met de hand worden opgehoogd. Dat is de bedoeling: het getal in de
//! weigering is de enige plek waar de omvang zichtbaar wordt vóórdat hij is
//! betaald.
//!
//! ## Stopregels, en waarom ze vóór het getal komen
//!
//! - **Delegatie.** Wijst een kant naar een ministeriële regeling of een
//!   beleidsregel, dan levert dat een open term op en houdt het daar op
//!   (RFC-026). Er zijn duizenden van die documenten en het gat dat overblijft
//!   is een bekend gat.
//! - **Buiten het corpus.** Een wet die het corpus niet heeft is een bekend gat
//!   en geen te volgen kant.
//! - **Verwijzing zonder artikel.** "de Wet langdurige zorg" of "afdeling
//!   3.3.1" noemt geen artikel, dus er is niets om naartoe te lopen. Op diepte 3
//!   staan er ruim drieduizend van; ze volgen zou het wetboek oogsten.
//!
//! ## Algemene wetten zijn gewone wetten
//!
//! Er stond hier een vierde stopregel: een handgeschreven lijst "kaderwetten"
//! (de Awb en de Awir) die als kaart naast de diepte meegingen en waar niet in
//! of uit werd gelopen. Die uitzondering is geschrapt. Gemeten op het corpus
//! komt de Awir vanaf de Wet op de zorgtoeslag op diepte 1 gewoon binnen met
//! de artikelen die op de handgeschreven kaart stonden, dus daar berekende de
//! graaf al wat de lijst wilde afdwingen, en "niet inlopen" hield nog wel de
//! producent buiten het plan die deepest-first eerder vertaald moest worden dan
//! zijn lezer. De Awb heeft het omgekeerde probleem: de zorgtoeslag citeert hem
//! zelf nergens, en de besluit-machinerie (beslistermijnen, bezwaar) komt op
//! geen enkele diepte via verwijzingen binnen; op diepte 2 is het enige
//! Awb-artikel 4:3, als bijvangst. Een kaart verscheen wel, maar alleen omdat
//! meegetrokken Zvw-artikelen toevallig een Awb-kant hebben; hij ontstond op
//! een kant die de traversal tegenkwam, hing dus af van toevallige citaten
//! elders in de sluiting, en zei niets over toepasselijkheid. Een wet die
//! werkt zonder geciteerd te worden is een relatie met een bron in de wettekst
//! (de reikwijdtebepaling, als `applies-to`-kant) en een trigger in het schema
//! (`legal_character`), geen wetscategorie in de planner. Zie RFC-026.
//!
//! ## Definitie tegenover berekening
//!
//! RFC-026 typeert kanten: `uses-definition` sleept één artikel mee,
//! `computes-with` trekt een keten. Uit de kale tekst is dat onderscheid maar
//! half te maken.
//!
//! Wat wel werkt is het **doel**: een artikel waarvan de tekst "wordt verstaan
//! onder" bevat is een begripsbepaling, en daar houdt de wandeling op.
//! [`StopRules::definitions_are_leaves`] doet dat en staat aan. In een
//! prototype van deze traversal scheelde die regel ruim een tiende van de
//! sluiting op diepte 3; met deze code is dat verschil nog niet los gemeten,
//! want er is nog geen schakelaar om hem uit te zetten.
//!
//! Wat niet werkt is de **kant**. De aanhef "bedoeld in" dekt allebei de
//! gevallen: "de persoon, bedoeld in artikel 1, onder f" is definitorisch en
//! "de premie, bedoeld in afdeling 3.3.1" vraagt een waarde. Er is geen woord
//! in de brontekst dat de twee scheidt. De diepte telt daarom over alle kanten,
//! en de consequentie staat in de tabel hierboven: diepte 1 haalt 624 artikelen
//! binnen waar een echte `computes-with`-sluiting er veel minder zou halen.
//! Het onderscheid komt terug zodra de kanten getypeerd in het corpus staan in
//! plaats van in de tekst.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

use super::refgraph::Graph;

/// Where a law lives and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawEntry {
    /// BWB identifier.
    pub bwb_id: String,
    /// `$id` of the law, as the corpus names it.
    pub law_id: String,
    /// Path of the newest version, relative to the corpus root.
    pub path: String,
    /// `regulatory_layer`, which decides whether an edge to it is delegation.
    pub layer: String,
}

impl LawEntry {
    /// Whether an edge into this law is a delegation edge, which ends the
    /// traversal with a known gap.
    #[must_use]
    pub fn is_delegated_layer(&self) -> bool {
        matches!(
            self.layer.as_str(),
            "MINISTERIELE_REGELING" | "BELEIDSREGEL" | "CIRCULAIRE"
        )
    }
}

/// Every law in a corpus, keyed by BWB identifier.
///
/// Built by reading the head of the newest version of every law directory —
/// the four fields above are all in the first dozen lines, so this never parses
/// a whole law. Four thousand directories is a fraction of a second.
#[derive(Debug, Clone, Default)]
pub struct LawIndex {
    by_bwb: BTreeMap<String, LawEntry>,
}

impl LawIndex {
    /// Scan `<root>/regulation` for law directories.
    pub fn scan(root: &Path) -> std::io::Result<Self> {
        let mut index = Self::default();
        let regulation = root.join("regulation");
        if !regulation.exists() {
            return Ok(index);
        }
        let mut stack = vec![regulation];
        while let Some(dir) = stack.pop() {
            let mut versions: Vec<PathBuf> = Vec::new();
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    stack.push(path);
                } else if is_version_file(&path) {
                    versions.push(path);
                }
            }
            versions.sort();
            let Some(newest) = versions.last() else {
                continue;
            };
            if let Some(entry) = read_head(newest, root) {
                index.by_bwb.entry(entry.bwb_id.clone()).or_insert(entry);
            }
        }
        Ok(index)
    }

    /// Look a law up by BWB identifier.
    #[must_use]
    pub fn get(&self, bwb_id: &str) -> Option<&LawEntry> {
        self.by_bwb.get(bwb_id)
    }

    /// How many laws the index holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_bwb.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_bwb.is_empty()
    }
}

/// A dated version file: `2026-01-01.yaml`. `status.yaml` and the dot-files the
/// enricher writes beside a law are not versions.
fn is_version_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.len() == "2026-01-01.yaml".len()
        && name.ends_with(".yaml")
        && name.as_bytes()[..10].iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Read `$id`, `bwb_id` and `regulatory_layer` off the head of a law file.
fn read_head(path: &Path, root: &Path) -> Option<LawEntry> {
    use std::io::BufRead;

    let file = std::fs::File::open(path).ok()?;
    let mut bwb_id = String::new();
    let mut law_id = String::new();
    let mut layer = String::new();
    for line in std::io::BufReader::new(file).lines().take(20) {
        let Ok(line) = line else { break };
        if let Some(rest) = line.strip_prefix("bwb_id:") {
            bwb_id = rest.trim().trim_matches(['\'', '"']).to_string();
        } else if let Some(rest) = line.strip_prefix("$id:") {
            law_id = rest.trim().trim_matches(['\'', '"']).to_string();
        } else if let Some(rest) = line.strip_prefix("regulatory_layer:") {
            layer = rest.trim().trim_matches(['\'', '"']).to_string();
        }
    }
    if bwb_id.is_empty() {
        return None;
    }
    Some(LawEntry {
        bwb_id,
        law_id,
        path: path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned(),
        layer,
    })
}

/// The bounds the traversal applies before the depth number does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopRules {
    /// Stop at an article whose text defines terms. See the module docs: it is
    /// the readable half of the `uses-definition` / `computes-with` split and
    /// removes about a tenth of the closure.
    pub definitions_are_leaves: bool,
}

impl Default for StopRules {
    fn default() -> Self {
        Self {
            definitions_are_leaves: true,
        }
    }
}

/// Why an edge was not followed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GapKind {
    /// The corpus does not have this law.
    OutsideCorpus,
    /// The target is a ministerial regulation or policy rule: the norm is
    /// delegated and an `open_term` is the finished answer.
    Delegated,
    /// The reference names a law, chapter or afdeling but no article.
    NoArticle,
    /// The depth ran out here.
    BeyondDepth,
}

/// One edge the traversal recorded instead of following.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gap {
    pub kind: GapKind,
    /// BWB identifier of the law the edge points at.
    pub bwb_id: String,
    /// How often this edge occurs in the closure.
    pub occurrences: usize,
}

/// One law's share of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// How many law jumps from the starting law.
    pub depth: usize,
    pub bwb_id: String,
    pub law_id: String,
    /// Path relative to the corpus root.
    pub path: String,
    /// Top-level articles on the hot path, in document order.
    pub articles: Vec<String>,
    /// Entries those articles hold, which is what an agent actually reads.
    pub entries: usize,
}

/// What `--depth` resolves to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Laws to enrich, deepest first: a producer is translated before the law
    /// that reads it, which is what makes the binding real instead of a guess.
    pub tasks: Vec<Task>,
    /// Edges recorded rather than followed.
    pub gaps: Vec<Gap>,
}

impl Plan {
    /// Total articles across every task.
    #[must_use]
    pub fn articles(&self) -> usize {
        self.tasks.iter().map(|t| t.articles.len()).sum()
    }

    /// Total entries across every task.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.tasks.iter().map(|t| t.entries).sum()
    }

    /// An error message when the plan is bigger than `limit` articles.
    ///
    /// A refusal and not a warning. Depth 2 on the Zorgverzekeringswet is four
    /// thousand articles across 156 laws; a run that starts that by accident
    /// costs days and nobody reads the warning that scrolled past.
    #[must_use]
    pub fn refuse_above(&self, limit: usize) -> Option<String> {
        if self.articles() <= limit {
            return None;
        }
        Some(format!(
            "this depth plans {} articles ({} entries) across {} laws, over the limit of {}. \
             Lower --depth, or raise the limit deliberately",
            self.articles(),
            self.entries(),
            self.tasks.len(),
            limit
        ))
    }

    /// One line per law, for the log and for the run's own record.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        self.tasks
            .iter()
            .map(|t| {
                format!(
                    "[diepte {}] {} — {} artikelen, {} entries",
                    t.depth,
                    t.law_id,
                    t.articles.len(),
                    t.entries
                )
            })
            .collect()
    }
}

/// Plan the closure from one article of one law.
///
/// `start_path` is relative to `root`. Errors only when the starting law cannot
/// be read; everything else the traversal meets and cannot follow becomes a
/// gap, because a closure that fails on the first unharvested law would never
/// produce a plan at all.
pub fn plan_closure(
    root: &Path,
    start_path: &str,
    start_articles: &[String],
    depth: usize,
    index: &LawIndex,
    rules: StopRules,
) -> std::result::Result<Plan, String> {
    let start_doc = read_law(&root.join(start_path))
        .ok_or_else(|| format!("cannot read the law at {start_path}"))?;
    let start_bwb = start_doc
        .get("bwb_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut laws: BTreeMap<String, Graph> = BTreeMap::new();
    let mut docs: BTreeMap<String, Value> = BTreeMap::new();
    laws.insert(start_bwb.clone(), Graph::scan(&start_doc));
    docs.insert(start_bwb.clone(), start_doc);

    for article in start_articles {
        if !laws
            .get(&start_bwb)
            .is_some_and(|g| g.articles.iter().any(|a| a == article))
        {
            return Err(format!(
                "law {start_path} has no article {article}; naming one it does not have is a \
                 mistake in the query and a plan of nothing would look like a plan of nothing to do"
            ));
        }
    }

    // Depth per law: breadth-first, so a law reached straight from the start
    // keeps depth 1 even when a longer path also arrives there.
    let mut law_depth: BTreeMap<String, usize> = BTreeMap::new();
    law_depth.insert(start_bwb.clone(), 0);
    let mut taken: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut gaps: BTreeMap<(GapKind, String), usize> = BTreeMap::new();

    // The queue holds (law, article, is_entry_point). An entry point is an
    // article the closure came in through; a definition article is a leaf only
    // when it was walked into, never when it was asked for by name.
    let mut queue: VecDeque<(String, String, bool)> = VecDeque::new();
    for article in start_articles {
        queue.push_back((start_bwb.clone(), article.clone(), true));
        taken
            .entry(start_bwb.clone())
            .or_default()
            .insert(article.clone());
    }

    while let Some((bwb, article, entry_point)) = queue.pop_front() {
        let Some(graph) = laws.get(&bwb) else {
            continue;
        };
        let here = law_depth.get(&bwb).copied().unwrap_or(0);

        if rules.definitions_are_leaves
            && !entry_point
            && docs
                .get(&bwb)
                .is_some_and(|doc| is_definition_article(doc, &article))
        {
            continue;
        }

        let mut internal: Vec<String> = Vec::new();
        let mut outward: Vec<(String, String)> = Vec::new();
        for (from, targets) in &graph.depends_on {
            if from.article != article || from.bwb_id != bwb {
                continue;
            }
            for target in targets {
                if target.bwb_id == bwb {
                    internal.push(target.article.clone());
                } else {
                    outward.push((target.bwb_id.clone(), target.article.clone()));
                }
            }
        }
        for (from, target_law) in &graph.outward_law_only {
            if *from == article {
                *gaps
                    .entry((GapKind::NoArticle, target_law.clone()))
                    .or_default() += 1;
            }
        }

        // Free inside the law.
        for target in internal {
            if taken.entry(bwb.clone()).or_default().insert(target.clone()) {
                queue.push_back((bwb.clone(), target, false));
            }
        }

        for (target_law, target_article) in outward {
            let Some(entry) = index.get(&target_law) else {
                *gaps
                    .entry((GapKind::OutsideCorpus, target_law.clone()))
                    .or_default() += 1;
                continue;
            };
            if entry.is_delegated_layer() {
                *gaps
                    .entry((GapKind::Delegated, target_law.clone()))
                    .or_default() += 1;
                continue;
            }
            let there = here + 1;
            if there > depth {
                *gaps
                    .entry((GapKind::BeyondDepth, target_law.clone()))
                    .or_default() += 1;
                continue;
            }
            let known = law_depth.get(&target_law).copied();
            if known.is_none_or(|d| there < d) {
                law_depth.insert(target_law.clone(), there);
            }
            if !laws.contains_key(&target_law) {
                let Some(doc) = read_law(&root.join(&entry.path)) else {
                    *gaps
                        .entry((GapKind::OutsideCorpus, target_law.clone()))
                        .or_default() += 1;
                    continue;
                };
                laws.insert(target_law.clone(), Graph::scan(&doc));
                docs.insert(target_law.clone(), doc);
            }
            let exists = laws
                .get(&target_law)
                .is_some_and(|g| g.articles.contains(&target_article));
            if !exists {
                continue;
            }
            if taken
                .entry(target_law.clone())
                .or_default()
                .insert(target_article.clone())
            {
                queue.push_back((target_law, target_article, true));
            }
        }
    }

    // Deepest first, so a producer is translated before its reader.
    let mut tasks: Vec<Task> = Vec::new();
    for (bwb, articles) in taken {
        let Some(graph) = laws.get(&bwb) else {
            continue;
        };
        let entry = index.get(&bwb);
        let ordered: Vec<String> = graph
            .articles
            .iter()
            .filter(|a| articles.contains(*a))
            .cloned()
            .collect();
        let entries = graph
            .entries
            .iter()
            .filter(|e| ordered.iter().any(|a| a == super::refgraph::top_article(e)))
            .count();
        tasks.push(Task {
            depth: law_depth.get(&bwb).copied().unwrap_or(0),
            law_id: entry.map_or_else(String::new, |e| e.law_id.clone()),
            path: entry.map_or_else(String::new, |e| e.path.clone()),
            bwb_id: bwb,
            articles: ordered,
            entries,
        });
    }
    tasks.sort_by(|a, b| b.depth.cmp(&a.depth).then(a.law_id.cmp(&b.law_id)));

    Ok(Plan {
        tasks,
        gaps: gaps
            .into_iter()
            .map(|((kind, bwb_id), occurrences)| Gap {
                kind,
                bwb_id,
                occurrences,
            })
            .collect(),
    })
}

fn read_law(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_yaml_ng::from_str(&raw).ok()
}

/// Words a definition provision uses. The same list `checks` uses, because a
/// begripsbepaling is one thing and two definitions of it would drift.
const DEFINITION_WORDS: &[&str] = &[
    "wordt verstaan onder",
    "verstaan onder:",
    "wordt in deze",
    "wordt in dit",
];

fn is_definition_article(doc: &Value, article: &str) -> bool {
    let mut text = String::new();
    for entry in doc
        .get("articles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        if super::refgraph::top_article(number) != article {
            continue;
        }
        if let Some(t) = entry.get("text").and_then(Value::as_str) {
            text.push_str(&t.to_lowercase());
            text.push(' ');
        }
    }
    DEFINITION_WORDS.iter().any(|w| text.contains(w))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Three laws: A reads B and C straight, B reads C. C is therefore at
    /// depth 1 and not at 2, which is the rule that makes the depth a distance
    /// and not a path length.
    fn corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De hoogte volgt uit artikel 5 van wet B en uit artikel 9 van wet C.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '5'
      - id: ref2
        bwb_id: BWBR0000003
        artikel: '9'
  - number: '2'
    text: Een artikel dat niemand aanhaalt.
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_b/2026-01-01.yaml",
            r"$id: wet_b
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '5'
    text: Het bedrag wordt berekend met artikel 6 en met artikel 9 van wet C.
    references:
      - id: ref1
        bwb_id: BWBR0000003
        artikel: '9'
  - number: '6'
    text: Het tarief is tien procent.
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_c/2026-01-01.yaml",
            r"$id: wet_c
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '9'
    text: Het percentage wordt bij ministeriële regeling vastgesteld.
",
        );
        dir
    }

    fn write(dir: &tempfile::TempDir, rel: &str, body: &str) {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn plan_of(dir: &tempfile::TempDir, depth: usize) -> Plan {
        let index = LawIndex::scan(dir.path()).unwrap();
        plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["1".to_string()],
            depth,
            &index,
            StopRules::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_index_finds_every_law_by_its_bwb_number() {
        let dir = corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        assert_eq!(index.len(), 3);
        assert_eq!(index.get("BWBR0000002").unwrap().law_id, "wet_b");
        assert_eq!(
            index.get("BWBR0000002").unwrap().path,
            "regulation/nl/wet/wet_b/2026-01-01.yaml"
        );
    }

    #[test]
    fn depth_zero_stays_inside_the_starting_law() {
        let plan = plan_of(&corpus(), 0);
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].law_id, "wet_a");
        assert_eq!(plan.tasks[0].articles, vec!["1".to_string()]);
        // Article 2 is in the same law but not on the hot path: a step inside
        // a law is free, it is not automatic.
        assert!(!plan.tasks[0].articles.contains(&"2".to_string()));
        // Both outward edges are recorded as beyond the depth.
        assert_eq!(plan.gaps.len(), 2);
        assert!(plan.gaps.iter().all(|g| g.kind == GapKind::BeyondDepth));
    }

    /// The rule that gives the depth its meaning: A reaches C straight, so C
    /// costs one point, whatever the longer route through B would cost.
    #[test]
    fn a_law_reached_straight_from_the_start_costs_one_jump() {
        let plan = plan_of(&corpus(), 1);
        let depth_of = |id: &str| {
            plan.tasks
                .iter()
                .find(|t| t.law_id == id)
                .map(|t| t.depth)
                .unwrap()
        };
        assert_eq!(depth_of("wet_a"), 0);
        assert_eq!(depth_of("wet_b"), 1);
        assert_eq!(depth_of("wet_c"), 1, "A haalt C rechtstreeks, dus niveau 1");
    }

    /// Inside a law only the hot path is taken. Article 6 of B comes along
    /// because article 5 names it; article 2 of A stays out.
    #[test]
    fn a_step_inside_a_law_is_free_but_only_along_the_hot_path() {
        let plan = plan_of(&corpus(), 1);
        let b = plan.tasks.iter().find(|t| t.law_id == "wet_b").unwrap();
        assert_eq!(b.articles, vec!["5".to_string(), "6".to_string()]);
        let a = plan.tasks.iter().find(|t| t.law_id == "wet_a").unwrap();
        assert_eq!(a.articles, vec!["1".to_string()]);
    }

    /// Deepest first: wet_b and wet_c are translated before wet_a, so wet_a
    /// binds to names that already exist.
    #[test]
    fn the_plan_puts_producers_before_their_readers() {
        let plan = plan_of(&corpus(), 1);
        let depths: Vec<usize> = plan.tasks.iter().map(|t| t.depth).collect();
        assert_eq!(depths, vec![1, 1, 0]);
    }

    /// A law that declares itself applicable is an ordinary law in the plan.
    /// There used to be a designated list ("kaderwetten") whose members came
    /// along as a card beside the depth and were never walked into; a
    /// referenced producer must instead land in the plan as a task, deepest
    /// first, like any other law.
    #[test]
    fn a_self_declaring_law_is_an_ordinary_task_in_the_plan() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 3 van de algemene wet.
    references:
      - id: ref1
        bwb_id: BWBR0000009
        artikel: '3'
",
        );
        write(
            &dir,
            "regulation/nl/wet/algemene_wet/2026-01-01.yaml",
            r"$id: algemene_wet
regulatory_layer: WET
bwb_id: BWBR0000009
articles:
  - number: '3'
    text: Deze wet is van toepassing op elk besluit.
",
        );
        let plan = plan_of(&dir, 1);
        let task = plan
            .tasks
            .iter()
            .find(|t| t.law_id == "algemene_wet")
            .expect("de aangehaalde wet staat als taak in het plan");
        assert_eq!(task.depth, 1, "een wetssprong kost een punt, ook hier");
        assert_eq!(task.articles, vec!["3".to_string()]);
    }

    #[test]
    fn a_delegated_layer_ends_the_traversal_as_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 2 van de regeling.
    references:
      - id: ref1
        bwb_id: BWBR0000008
        artikel: '2'
",
        );
        write(
            &dir,
            "regulation/nl/wet/de_regeling/2026-01-01.yaml",
            r"$id: de_regeling
regulatory_layer: MINISTERIELE_REGELING
bwb_id: BWBR0000008
articles:
  - number: '2'
    text: Het percentage is drie.
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.gaps.len(), 1);
        assert_eq!(plan.gaps[0].kind, GapKind::Delegated);
    }

    #[test]
    fn a_law_the_corpus_does_not_have_is_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 2 van een wet die hier niet staat.
    references:
      - id: ref1
        bwb_id: BWBR9999999
        artikel: '2'
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.gaps[0].kind, GapKind::OutsideCorpus);
        assert_eq!(plan.gaps[0].bwb_id, "BWBR9999999");
    }

    /// A reference that names a law but no article has nothing to walk to. On
    /// the real corpus at depth 3 there are over three thousand of these.
    #[test]
    fn a_reference_without_an_article_is_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie de Wet langdurige zorg.
    references:
      - id: ref1
        bwb_id: BWBR0000002
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.gaps.len(), 1);
        assert_eq!(plan.gaps[0].kind, GapKind::NoArticle);
        assert_eq!(plan.tasks.len(), 1);
    }

    #[test]
    fn a_definition_article_is_a_leaf_when_walked_into() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: In deze wet wordt verstaan onder het begrip partner hetgeen artikel 4 bepaalt.
  - number: '2'
    text: De aanspraak volgt uit artikel 1.
  - number: '4'
    text: Een artikel dat alleen via de begripsbepaling bereikbaar is.
",
        );
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = |rules| {
            plan_closure(
                dir.path(),
                "regulation/nl/wet/wet_a/2026-01-01.yaml",
                &["2".to_string()],
                1,
                &index,
                rules,
            )
            .unwrap()
        };
        let stopping = plan(StopRules {
            definitions_are_leaves: true,
        });
        assert_eq!(
            stopping.tasks[0].articles,
            vec!["1".to_string(), "2".to_string()],
            "de begripsbepaling komt mee, maar sleept artikel 4 niet mee"
        );
        let walking = plan(StopRules {
            definitions_are_leaves: false,
        });
        assert_eq!(
            walking.tasks[0].articles,
            vec!["1".to_string(), "2".to_string(), "4".to_string()]
        );
    }

    /// Naming an article the law does not have fails loudly: a plan of nothing
    /// is indistinguishable from a plan with nothing to do.
    #[test]
    fn an_article_the_law_does_not_have_fails_the_plan() {
        let dir = corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        let error = plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["99".to_string()],
            1,
            &index,
            StopRules::default(),
        )
        .unwrap_err();
        assert!(error.contains("no article 99"), "{error}");
    }

    #[test]
    fn a_plan_over_the_limit_refuses_with_its_own_size() {
        let plan = plan_of(&corpus(), 1);
        assert!(plan.refuse_above(100).is_none());
        let refusal = plan.refuse_above(1).unwrap();
        assert!(refusal.contains("articles"), "{refusal}");
    }
}
