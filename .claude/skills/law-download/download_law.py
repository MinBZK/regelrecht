#!/usr/bin/env python3
"""
Download and convert a Dutch law from BWB to regelrecht YAML format.

Usage:
    python .claude/skills/law-download/download_law.py BWBR0033715 2025-02-12
    python .claude/skills/law-download/download_law.py BWBR0033715  # Uses latest version

Note: This is a standalone utility script. The law-download Claude skill
replaces this script for interactive use (it uses WebFetch instead of Python).
This script is kept as a reference and for batch/CI use.
"""

import sys
import re
from pathlib import Path
from datetime import datetime
import requests
import yaml
from lxml import etree

# XML Namespaces
BWB_NS = {"bwb": "http://www.overheid.nl/2011/BWB"}
WTI_NS = {
    "wti": "http://www.geonovum.nl/bwb-dl/1.0",
    "dcterms": "http://purl.org/dc/terms/",
}


def slugify(text):
    """Convert text to URL-friendly slug."""
    text = text.lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[-\s]+", "_", text)
    return text.strip("_")


def download_wti(bwbr_id):
    """Download WTI (metadata) file."""
    url = f"https://repository.officiele-overheidspublicaties.nl/bwb/{bwbr_id}/{bwbr_id}.WTI"
    print(f"Downloading WTI from: {url}")
    response = requests.get(url)
    response.raise_for_status()
    return etree.fromstring(response.content)


def download_toestand(bwbr_id, date):
    """Download Toestand (legal text) file."""
    url = f"https://repository.officiele-overheidspublicaties.nl/bwb/{bwbr_id}/{date}/xml/{bwbr_id}_{date}.xml"
    print(f"Downloading Toestand from: {url}")
    response = requests.get(url, allow_redirects=True)
    response.raise_for_status()
    return etree.fromstring(response.content)


def parse_wti_metadata(wti_tree):
    """Extract metadata from WTI XML."""
    metadata = {}

    NS = "http://www.geonovum.nl/bwb-dl/1.0"

    # Title
    citeertitel = wti_tree.find(f".//{{{NS}}}citeertitel[@status='officieel']")
    if citeertitel is not None:
        metadata["title"] = citeertitel.text

    # BWB ID (child element, not attribute)
    bwb_id_elem = wti_tree.find(f".//{{{NS}}}bwb-id")
    metadata["bwb_id"] = bwb_id_elem.text if bwb_id_elem is not None else None

    # Type (regulatory layer)
    VALID_LAYERS = {
        "GRONDWET", "WET", "AMVB", "KONINKLIJK_BESLUIT", "MINISTERIELE_REGELING",
        "BELEIDSREGEL", "EU_VERORDENING", "EU_RICHTLIJN", "VERDRAG",
        "UITVOERINGSBELEID", "GEMEENTELIJKE_VERORDENING", "PROVINCIALE_VERORDENING",
    }
    soort = wti_tree.find(f".//{{{NS}}}soort-regeling")
    if soort is not None:
        soort_text = soort.text.lower()
        type_mapping = {
            "wet": "WET",
            "amvb": "AMVB",
            "algemene maatregel van bestuur": "AMVB",
            "ministeriele regeling": "MINISTERIELE_REGELING",
            "ministeriële regeling": "MINISTERIELE_REGELING",
            "beleidsregel": "BELEIDSREGEL",
            "koninklijk besluit": "KONINKLIJK_BESLUIT",
        }
        layer = type_mapping.get(soort_text, soort_text.upper().replace(" ", "_"))
        if layer not in VALID_LAYERS:
            raise ValueError(
                f"'{soort_text}' maps to '{layer}' which is not a valid "
                f"schema v0.5.0 regulatory_layer. Map manually to e.g. AMVB."
            )
        metadata["regulatory_layer"] = layer

    # Guard: bwb_id is required for national laws
    NATIONAL_LAYERS = {"WET", "AMVB", "MINISTERIELE_REGELING", "GRONDWET"}
    if metadata.get("bwb_id") is None and metadata.get("regulatory_layer") in NATIONAL_LAYERS:
        raise ValueError(f"bwb-id element not found in WTI XML (namespace: {NS})")

    # Publication date
    pub_date = wti_tree.find(f".//{{{NS}}}publicatiedatum")
    if pub_date is not None:
        metadata["publication_date"] = pub_date.text

    return metadata


def extract_text_from_element(elem):
    """Extract text from XML element, preserving structure."""
    if elem is None:
        return ""

    text_parts = []

    # Get direct text
    if elem.text:
        text_parts.append(elem.text.strip())

    # Process child elements
    for child in elem:
        if child.tag.endswith("al"):  # Paragraph
            child_text = extract_text_from_element(child)
            if child_text:
                text_parts.append(child_text)
        elif child.tag.endswith("lijst"):  # List
            for li in child.findall(".//li"):
                li_text = extract_text_from_element(li)
                if li_text:
                    text_parts.append(f"- {li_text}")
        elif child.tag.endswith("nadruk"):  # Emphasis
            child_text = extract_text_from_element(child)
            nadruk_type = child.get("type", "")
            if nadruk_type == "vet":
                text_parts.append(f"**{child_text}**")
            else:
                text_parts.append(f"*{child_text}*")
        elif child.tag.endswith("extref"):  # External reference
            ref_text = child.text or ""
            ref_url = child.get("doc", "")
            if ref_url:
                text_parts.append(f"[{ref_text}]({ref_url})")
            else:
                text_parts.append(ref_text)
        else:
            # Recursive for other elements
            child_text = extract_text_from_element(child)
            if child_text:
                text_parts.append(child_text)

        # Get tail text
        if child.tail:
            text_parts.append(child.tail.strip())

    return " ".join(part for part in text_parts if part)


def parse_articles(toestand_tree, bwbr_id, date):
    """Extract articles from Toestand XML."""
    articles = []

    # Find all artikel elements (no namespace)
    artikel_elements = toestand_tree.findall(".//artikel")

    print(f"Found {len(artikel_elements)} articles")

    for artikel in artikel_elements:
        # Get article number - try label attribute first, then nr element
        article_number = artikel.get("label", "")
        if not article_number:
            nr_elem = artikel.find(".//nr")
            if nr_elem is not None:
                article_number = nr_elem.text

        if not article_number:
            continue

        # Extract all text from this article
        article_text = extract_text_from_element(artikel)

        # Generate URL
        article_url = (
            f"https://wetten.overheid.nl/{bwbr_id}/{date}#Artikel{article_number}"
        )

        articles.append(
            {"number": article_number, "text": article_text, "url": article_url}
        )

    return articles


def generate_yaml(metadata, articles, effective_date):
    """Generate YAML law file."""
    # Generate law ID from title
    bwb_id = metadata.get("bwb_id")
    law_id = slugify(metadata.get("title", bwb_id or "unknown"))

    # Create YAML structure matching schema v0.5.0
    law_data = {
        "$schema": "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.0/schema.json",
        "$id": law_id,
        "name": metadata.get("title", law_id),
        "regulatory_layer": metadata.get("regulatory_layer", "WET"),
        "publication_date": metadata.get("publication_date", effective_date),
        "valid_from": effective_date,
        "url": f"https://wetten.overheid.nl/{bwb_id}/{effective_date}",
        "articles": articles,
    }
    # Only include bwb_id if present (required for national laws, optional otherwise)
    if bwb_id is not None:
        law_data["bwb_id"] = bwb_id

    return law_id, law_data


def save_yaml(law_id, law_data, regulatory_layer, effective_date):
    """Save YAML file to appropriate directory."""
    # Determine directory structure
    layer_dir = regulatory_layer.lower()
    output_dir = Path(f"corpus/regulation/nl/{layer_dir}/{law_id}")
    output_dir.mkdir(parents=True, exist_ok=True)

    output_file = output_dir / f"{effective_date}.yaml"

    print(f"Saving to: {output_file}")

    with open(output_file, "w", encoding="utf-8") as f:
        yaml.dump(
            law_data,
            f,
            allow_unicode=True,
            sort_keys=False,
            default_flow_style=False,
            width=100,
        )

    return output_file


def main():
    if len(sys.argv) < 2:
        print("Usage: python .claude/skills/law-download/download_law.py BWBR_ID [DATE]")
        print()
        print("Example:")
        print("  python .claude/skills/law-download/download_law.py BWBR0033715 2025-02-12")
        sys.exit(1)

    bwbr_id = sys.argv[1]
    effective_date = sys.argv[2] if len(sys.argv) > 2 else None

    # If no date provided, use today
    if not effective_date:
        effective_date = datetime.now().strftime("%Y-%m-%d")
        print(f"No date provided, using: {effective_date}")

    print(f"Processing {bwbr_id} for date {effective_date}")
    print()

    try:
        # Download files
        wti_tree = download_wti(bwbr_id)
        toestand_tree = download_toestand(bwbr_id, effective_date)

        # Parse metadata
        print("Parsing metadata...")
        metadata = parse_wti_metadata(wti_tree)
        print(f"   Title: {metadata.get('title', 'Unknown')}")
        print(f"   Type: {metadata.get('regulatory_layer', 'Unknown')}")

        # Parse articles
        print("Parsing articles...")
        articles = parse_articles(toestand_tree, bwbr_id, effective_date)

        # Generate YAML
        print("Generating YAML...")
        law_id, law_data = generate_yaml(metadata, articles, effective_date)

        # Save file
        output_file = save_yaml(
            law_id, law_data, metadata.get("regulatory_layer", "WET"), effective_date
        )

        print()
        print("Success!")
        print(f"   Saved to: {output_file}")
        print(f"   Articles: {len(articles)}")
        print()
        print("Next steps:")
        print(f"1. Validate: just validate {output_file}")
        print(
            "2. Interpret: Use the law-interpret skill to add machine_readable sections"
        )

    except Exception as e:
        print(f"Error: {e}")
        import traceback

        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
