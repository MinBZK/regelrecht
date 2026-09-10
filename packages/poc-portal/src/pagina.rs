//! The two pages the portal renders itself: the index and the login screen.
//!
//! Server-rendered HTML rather than a Vue app, for one reason: the login screen
//! has to work before any PoC bundle is allowed to load. A build step for two
//! pages would also mean a fourth npm workspace that exists only to render a
//! card list.
//!
//! The markup follows the design system's own composition — the card grid is
//! the one `docs/src/components/LandingSections.astro` uses, the card itself is
//! `docs/src/components/LinkCard.astro`. The design-system bundle is built into
//! the image and served from `/_assets/`; nothing is fetched from a CDN, which
//! is what lets the CSP stay on `'self'`.

use crate::registry::{Poc, Registry};

/// Minimal HTML escaping for text that lands in element content or an
/// attribute value. The register is ours, but it is data in a file, and a
/// stray `&` in a title should render rather than break the page.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn omhulsel(titel: &str, inhoud: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="nl">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{titel}</title>
<link rel="stylesheet" href="/_assets/nldd.css">
<script type="module" src="/_assets/nldd.js"></script>
</head>
<body>
<nldd-page>
{inhoud}
</nldd-page>
</body>
</html>
"#,
        titel = esc(titel),
        inhoud = inhoud,
    )
}

fn kaart(poc: &Poc) -> String {
    let tags = poc
        .tags
        .iter()
        .map(|t| {
            format!(
                r#"<nldd-tag color="accent" size="md">{}</nldd-tag>"#,
                esc(t)
            )
        })
        .collect::<Vec<_>>()
        .join("\n            ");

    format!(
        r#"        <nldd-card accessible-label="{titel}">
          <nldd-container padding="20" padding-bottom="12">
            <nldd-title size="3"><h3>{titel}</h3></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-container layout="wrap" gap="8">
            {tags}
            </nldd-container>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-rich-text><p>{samenvatting}</p></nldd-rich-text>
          </nldd-container>
          <nldd-container slot="footer" padding="20" padding-top="0">
            <nldd-button variant="secondary" width="full" href="/{slug}/"
              text="Openen" accessible-label="Open {titel}"></nldd-button>
          </nldd-container>
        </nldd-card>"#,
        titel = esc(&poc.titel),
        samenvatting = esc(&poc.samenvatting),
        slug = esc(&poc.slug),
        tags = tags,
    )
}

/// The index: every PoC in the register as a card.
///
/// Public on purpose. A list of titles gives nothing away, and it is exactly
/// the page you send someone before you send them a password.
pub fn index(registry: &Registry) -> String {
    let kaarten = registry
        .pocs
        .iter()
        .map(kaart)
        .collect::<Vec<_>>()
        .join("\n");

    let inhoud = format!(
        r#"  <nldd-simple-section sm-padding-block="32" md-padding-block="64">
    <nldd-title size="1"><h1>Proof-of-concepts</h1></nldd-title>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-rich-text>
      <p>Uitwerkingen van wet- en regelgeving als machine-uitvoerbare modellen,
      elk met een eigen demo-omgeving. Ze tonen wat er met de
      regelrecht-engine mogelijk is; het zijn geen productiesystemen en er
      kunnen geen rechten aan worden ontleend.</p>
      <p>Elke omgeving zit achter een eigen wachtwoord.</p>
    </nldd-rich-text>
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-collection layout="grid" item-width="320px">
{kaarten}
    </nldd-collection>
  </nldd-simple-section>"#
    );

    omhulsel("Proof-of-concepts — regelrecht", &inhoud)
}

/// The login screen for one PoC.
///
/// Served with 401, not a redirect: a deep link keeps its address, so signing
/// in lands the visitor where they were going instead of on the index.
pub fn inloggen(poc: &Poc, pad: &str, mislukt: bool) -> String {
    // `critical` rather than a quieter variant: the banner then carries
    // role="alert", which is what a screen reader needs after a failed attempt.
    let melding = if mislukt {
        r#"<nldd-banner variant="critical" text="Onjuist wachtwoord"
            supporting-text="Controleer het wachtwoord en probeer het opnieuw."></nldd-banner>
          <nldd-spacer size="16"></nldd-spacer>"#
    } else {
        ""
    };

    let inhoud = format!(
        r#"  <nldd-simple-section width="480px" sm-padding-block="32" md-padding-block="64">
      <nldd-title size="1"><h1>{titel}</h1></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-rich-text><p>{samenvatting}</p></nldd-rich-text>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-card accessible-label="Wachtwoord">
        <nldd-container padding="24">
          {melding}
          <nldd-rich-text>
            <p>Deze omgeving is afgeschermd. Vul het wachtwoord in dat je bij de
            uitnodiging hebt gekregen.</p>
          </nldd-rich-text>
          <nldd-spacer size="16"></nldd-spacer>
          <form method="post" action="/_toegang/{slug}">
            <input type="hidden" name="verder" value="{pad}">
            <nldd-form-field label="Wachtwoord">
              <nldd-password-field name="wachtwoord" autocomplete="current-password"
                required></nldd-password-field>
            </nldd-form-field>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-button type="submit" variant="primary" text="Toegang"></nldd-button>
          </form>
        </nldd-container>
      </nldd-card>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-link href="/" text="Terug naar het overzicht"></nldd-link>
  </nldd-simple-section>"#,
        titel = esc(&poc.titel),
        samenvatting = esc(&poc.samenvatting),
        slug = esc(&poc.slug),
        pad = esc(pad),
        melding = melding,
    );

    omhulsel(&format!("{} — wachtwoord", poc.titel), &inhoud)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        Registry::from_yaml(include_str!("../../../pocs/registry.yaml")).expect("valid")
    }

    #[test]
    fn the_index_lists_every_poc_in_the_register() {
        let r = registry();
        let html = index(&r);
        for poc in &r.pocs {
            assert!(html.contains(&poc.titel), "missing {}", poc.slug);
            assert!(html.contains(&format!(r#"href="/{}/""#, poc.slug)));
        }
    }

    #[test]
    fn the_index_does_not_leak_a_password_or_env_name() {
        let html = index(&registry());
        assert!(
            !html.contains("POC_PW"),
            "the index must not name the secrets"
        );
    }

    #[test]
    fn the_login_form_posts_to_the_slugs_own_endpoint() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = inloggen(poc, "/napp/aanvrager/", false);
        assert!(html.contains(r#"action="/_toegang/napp""#));
        assert!(html.contains(r#"value="/napp/aanvrager/""#));
        assert!(html.contains(r#"<nldd-password-field name="wachtwoord""#));
    }

    /// Every custom element these pages use, checked against the design
    /// system's own API (`@nldd/design-system/dist/components/**/*.d.ts`).
    ///
    /// A web component with an attribute it does not know renders nothing and
    /// says nothing — no console error, no failed build. This list was wrong in
    /// four places on the first pass (`nldd-alert` does not exist, `nldd-card`
    /// has no `background`, `nldd-container` has no `max-width`, and
    /// `nldd-form-field` has no `for`), so it is worth pinning what survived
    /// that check.
    #[test]
    fn only_elements_that_exist_in_the_design_system_are_used() {
        const BESTAAT: &[&str] = &[
            "nldd-page",
            "nldd-simple-section",
            "nldd-collection",
            "nldd-container",
            "nldd-card",
            "nldd-title",
            "nldd-rich-text",
            "nldd-spacer",
            "nldd-tag",
            "nldd-button",
            "nldd-link",
            "nldd-banner",
            "nldd-form-field",
            "nldd-password-field",
        ];
        let r = registry();
        let mut html = index(&r);
        html.push_str(&inloggen(r.get("napp").expect("napp"), "/napp/", true));

        for (i, _) in html.match_indices("<nldd-") {
            let naam: String = html[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect();
            assert!(
                BESTAAT.contains(&naam.as_str()),
                "{naam} is not in the design system — check its .d.ts before using it",
            );
        }
    }

    #[test]
    fn the_error_is_only_shown_after_a_failed_attempt() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        assert!(!inloggen(poc, "/napp/", false).contains("Onjuist wachtwoord"));
        assert!(inloggen(poc, "/napp/", true).contains("Onjuist wachtwoord"));
    }

    #[test]
    fn markup_from_the_register_is_escaped() {
        // The register is ours, but it is a data file: a title with a quote or
        // a bracket must render, not break out of the attribute it sits in.
        let poc = Poc {
            slug: "x".into(),
            titel: r#"A "quoted" <b>title</b>"#.into(),
            samenvatting: "5 > 3 & rising".into(),
            soort: crate::registry::Soort::Statisch,
            bron: Some("b".into()),
            upstream: None,
            corpus: vec![],
            assistent: false,
            tags: vec!["<script>".into()],
        };
        let html = kaart(&poc);
        assert!(!html.contains("<b>title</b>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&amp; rising"));
    }

    #[test]
    fn the_return_path_is_escaped_too() {
        // `verder` comes off the request line, so it is the one value here a
        // visitor controls.
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = inloggen(poc, r#"/napp/"><script>alert(1)</script>"#, false);
        assert!(!html.contains("<script>alert(1)"));
    }
}
