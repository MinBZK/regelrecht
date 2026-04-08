//! End-to-end integration tests for the harvester pipeline.
//!
//! Tests the complete pipeline from XML parsing to YAML generation
//! using fixture data from the Wet op de zorgtoeslag (BWBR0018451).

use std::fs;
use std::path::Path;

use regelrecht_harvester::types::{Article, Law, Preamble, RegulatoryLayer};
use regelrecht_harvester::wti::parse_wti_metadata;
use regelrecht_harvester::yaml::generate_yaml;

/// Load fixture file content.
fn load_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("zorgtoeslag")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to load {}: {}", path.display(), e))
}

/// Parsed content from integration test.
struct ParsedContent {
    preamble: Option<Preamble>,
    articles: Vec<Article>,
}

/// Run the harvester pipeline on zorgtoeslag fixtures.
fn run_pipeline() -> Law {
    let wti_xml = load_fixture("wti.xml");
    let content_xml = load_fixture("content.xml");

    // Parse WTI metadata
    let wti_doc = roxmltree::Document::parse(&wti_xml).expect("Failed to parse WTI XML");
    let wti_result = parse_wti_metadata(&wti_doc);

    // Parse articles from content
    let content_doc =
        roxmltree::Document::parse(&content_xml).expect("Failed to parse content XML");
    let parsed = parse_content(&content_doc, &wti_result.metadata.bwb_id, "2025-01-01");

    Law {
        metadata: wti_result.metadata,
        preamble: parsed.preamble,
        articles: parsed.articles,
        warnings: wti_result.warnings,
    }
}

/// Parse content from XML document (preamble + articles).
fn parse_content(doc: &roxmltree::Document<'_>, bwb_id: &str, date: &str) -> ParsedContent {
    use regelrecht_harvester::config::wetten_url;
    use regelrecht_harvester::splitting::{
        create_dutch_law_hierarchy, LeafSplitStrategy, SplitContext, SplitEngine,
    };
    use regelrecht_harvester::xml::{find_by_path, find_children, get_tag_name, get_text};

    let mut articles = Vec::new();
    let mut preamble = None;

    // Extract aanhef as preamble
    if let Some(aanhef) = doc
        .descendants()
        .find(|n| n.is_element() && get_tag_name(*n) == "aanhef")
    {
        let mut parts: Vec<String> = Vec::new();

        if let Some(wij) = find_children(aanhef, "wij").next() {
            if let Some(text) = wij.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
        }

        if let Some(considerans) = find_children(aanhef, "considerans").next() {
            for al in considerans
                .descendants()
                .filter(|n| n.is_element() && get_tag_name(*n) == "considerans.al")
            {
                let text = extract_simple_text(al);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }

        if let Some(afkondiging) = find_children(aanhef, "afkondiging").next() {
            for al in find_children(afkondiging, "al") {
                let text = extract_simple_text(al);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }

        if !parts.is_empty() {
            preamble = Some(Preamble {
                text: parts.join("\n\n"),
                url: wetten_url(bwb_id, Some(date), Some("Aanhef"), None, None, None),
            });
        }
    }

    // Create split engine
    let hierarchy = create_dutch_law_hierarchy();
    let engine = SplitEngine::new(hierarchy, LeafSplitStrategy);

    // Find all artikel elements
    for artikel in doc
        .descendants()
        .filter(|n| n.is_element() && get_tag_name(*n) == "artikel")
    {
        let artikel_nr = if let Some(nr_node) = find_by_path(artikel, "kop/nr") {
            get_text(nr_node)
        } else if let Some(label) = artikel.attribute("label") {
            label.strip_prefix("Artikel ").unwrap_or(label).to_string()
        } else {
            continue;
        };

        let artikel_nr_url = artikel_nr.replace(' ', "_");
        let base_url = wetten_url(bwb_id, Some(date), Some(&artikel_nr_url), None, None, None);
        let context = SplitContext::new(bwb_id, date, base_url);

        let components = engine.split(artikel, context);
        for component in components {
            articles.push(component.to_article());
        }
    }

    ParsedContent { preamble, articles }
}

/// Simple text extraction from a node.
fn extract_simple_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut text = String::new();

    if let Some(t) = node.text() {
        text.push_str(t);
    }

    for child in node.children() {
        if child.is_element() {
            text.push_str(&extract_simple_text(child));
        }
        if let Some(tail) = child.tail() {
            text.push_str(tail);
        }
    }

    text.trim().to_string()
}

#[test]
fn test_pipeline_article_count() {
    let law = run_pipeline();

    // Expected: 35 artikel components (preamble is now separate)
    assert_eq!(
        law.articles.len(),
        35,
        "Expected 35 articles, got {}",
        law.articles.len()
    );

    // Preamble should be present
    assert!(law.preamble.is_some(), "Should have preamble");
}

#[test]
fn test_pipeline_metadata() {
    let law = run_pipeline();

    assert_eq!(law.metadata.bwb_id, "BWBR0018451");
    assert_eq!(law.metadata.title, "Wet op de zorgtoeslag");
    assert_eq!(law.metadata.regulatory_layer, RegulatoryLayer::Wet);
}

#[test]
fn test_pipeline_preamble() {
    let law = run_pipeline();

    let preamble = law.preamble.as_ref();
    assert!(preamble.is_some(), "Should have preamble");

    let preamble = preamble.unwrap();
    assert!(
        preamble.text.contains("Wij Beatrix"),
        "Preamble should contain 'Wij Beatrix'"
    );
    assert!(
        preamble.text.contains("zorgtoeslag") || preamble.text.contains("zorgverzekering"),
        "Preamble should mention zorgtoeslag or zorgverzekering"
    );
    assert!(
        preamble.url.contains("Aanhef"),
        "Preamble URL should contain 'Aanhef'"
    );
}

#[test]
fn test_pipeline_article_1_components() {
    let law = run_pipeline();

    // Article 1 should be split into multiple components
    let art_1_1 = law.articles.iter().find(|a| a.number == "1.1");
    assert!(art_1_1.is_some(), "Should have article 1.1 (intro)");

    let art_1_1_a = law.articles.iter().find(|a| a.number == "1.1.a");
    assert!(art_1_1_a.is_some(), "Should have article 1.1.a");

    if let Some(art) = art_1_1 {
        assert!(
            art.text.contains("In deze wet"),
            "Article 1.1 should contain 'In deze wet'"
        );
    }

    if let Some(art) = art_1_1_a {
        assert!(
            art.text.contains("Onze Minister"),
            "Article 1.1.a should contain 'Onze Minister'"
        );
    }
}

#[test]
fn test_pipeline_article_urls() {
    let law = run_pipeline();

    let art_1_1 = law.articles.iter().find(|a| a.number == "1.1").unwrap();
    assert_eq!(
        art_1_1.url,
        "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1"
    );

    if let Some(art_8) = law.articles.iter().find(|a| a.number == "8") {
        assert_eq!(
            art_8.url,
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel8"
        );
    }
}

#[test]
fn test_pipeline_citeertitel() {
    let law = run_pipeline();

    // Article 8 should contain the citeertitel
    let art_8 = law.articles.iter().find(|a| a.number == "8");
    assert!(art_8.is_some(), "Should have article 8");

    if let Some(art) = art_8 {
        let text_lower = art.text.to_lowercase();
        assert!(
            text_lower.contains("aangehaald als") || text_lower.contains("citeertitel"),
            "Article 8 should mention citation"
        );
        assert!(
            text_lower.contains("zorgtoeslag"),
            "Article 8 should mention zorgtoeslag"
        );
    }
}

#[test]
fn test_yaml_generation() {
    let law = run_pipeline();
    let yaml = generate_yaml(&law, "2025-01-01").expect("Failed to generate YAML");

    // Basic structure checks
    assert!(
        yaml.starts_with("---\n"),
        "YAML should start with document marker"
    );
    assert!(yaml.contains("$schema:"), "YAML should contain $schema");
    assert!(
        yaml.contains("$id: wet_op_de_zorgtoeslag"),
        "YAML should contain correct $id"
    );
    assert!(
        yaml.contains("regulatory_layer: WET"),
        "YAML should contain regulatory_layer"
    );
    assert!(
        yaml.contains("bwb_id: BWBR0018451"),
        "YAML should contain bwb_id"
    );
}

#[test]
fn test_yaml_validates_structure() {
    let law = run_pipeline();
    let yaml = generate_yaml(&law, "2025-01-01").expect("Failed to generate YAML");

    // Parse as YAML to verify it's valid
    let parsed: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yaml).expect("Generated YAML should be valid");

    // Check required fields exist
    assert!(parsed.get("$schema").is_some(), "Should have $schema");
    assert!(parsed.get("$id").is_some(), "Should have $id");
    assert!(parsed.get("articles").is_some(), "Should have articles");

    // Check articles is an array
    let articles = parsed.get("articles").unwrap();
    assert!(articles.is_sequence(), "articles should be an array");
}

#[test]
fn test_references_extracted() {
    let law = run_pipeline();

    // Find an article that should have references (article 1.1.b references Zorgverzekeringswet)
    let art_with_refs = law.articles.iter().find(|a| !a.references.is_empty());

    assert!(
        art_with_refs.is_some(),
        "At least one article should have references"
    );

    if let Some(art) = art_with_refs {
        // References should point to Zorgverzekeringswet (BWBR0018450)
        let has_zvw_ref = art.references.iter().any(|r| r.bwb_id == "BWBR0018450");
        assert!(
            has_zvw_ref,
            "Should have reference to Zorgverzekeringswet (BWBR0018450)"
        );
    }
}
