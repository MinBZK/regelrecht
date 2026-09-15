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

/// The page shell.
///
/// `nldd-app-view` is the design system's required root and is not optional
/// chrome: it is the element carrying `min-height: 100dvh`. `nldd-page` only
/// has `height: 100%`, which resolves to nothing without a parent that has a
/// height — so leaving app-view out ends the page background wherever the
/// content happens to stop, with the viewport bare underneath.
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
<nldd-app-view>
<nldd-page>
{inhoud}
</nldd-page>
</nldd-app-view>
</body>
</html>
"#,
        titel = esc(titel),
        inhoud = inhoud,
    )
}

/// One card on the public index.
///
/// Carries the public title and summary and nothing else. The subject tags name
/// the department that owns the dossier and the status says how far along it is;
/// both belong to the case, so both wait behind the password. The status is not
/// softened away by leaving it out here — every page that shows an actual
/// outcome still carries it, and those are all behind the gate.
fn kaart(poc: &Poc) -> String {
    format!(
        r#"        <nldd-card accessible-label="{titel}">
          <nldd-container padding="20" padding-bottom="12">
            <nldd-title size="3"><h3>{titel}</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-rich-text><p>{samenvatting}</p></nldd-rich-text>
          </nldd-container>
          <nldd-container slot="footer" padding="20" padding-top="0">
            <nldd-button variant="secondary" width="full" href="/{slug}/"
              start-icon="lock" text="Openen"
              accessible-label="Open {titel}"></nldd-button>
          </nldd-container>
        </nldd-card>"#,
        titel = esc(&poc.titel),
        samenvatting = esc(&poc.samenvatting),
        slug = esc(&poc.slug),
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
      <p>Verkenningen rond wet- en regelgeving, elk in een eigen omgeving. Het
      zijn geen productiesystemen en er kunnen geen rechten aan worden
      ontleend.</p>
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

/// The strip the portal injects into a PoC's own pages.
///
/// This is now the only place the status and the voorbehoud are rendered. They
/// used to appear on the login screen too, but that screen is public, and both
/// of them describe the case: which dossier, how far along, what does not hold
/// yet. The reason they exist is unchanged — a screenshot from inside a PoC
/// travels without any surrounding text — and every page that shows an outcome
/// sits behind the password, so the strip reaches all of them.
///
/// A PoC is a separate application that knows nothing about this portal, so the
/// notice cannot live in its source without editing all three of them (and
/// every one added later). Injecting it here means a new PoC carries it by
/// arriving in the register, which is the whole point of the register.
///
/// Deliberately not the `nldd-banner` used elsewhere: this lands inside another
/// app's page, and mounting a web component there would depend on that app
/// having loaded the same design-system build. Plain HTML with inline styles in
/// a `data-poc-portaal` element cannot collide with the PoC's own markup and
/// needs nothing loaded.
///
/// The inline `style` attribute is why `POC_CSP` keeps `style-src
/// 'unsafe-inline'` — which it needs for the NDD components anyway.
pub fn voorbehoud_strip(poc: &Poc) -> String {
    // One line each: this is spliced into another document, and a multi-line
    // raw string would carry this file's indentation into it.
    //
    // The strip stays one row high. Its job here is to be present on every
    // screenshot and every deep link, not to repeat the argument; the full text
    // is in the `title` attribute for anyone who wants it in full.
    let stijl = "position:sticky;top:0;z-index:2147483647;display:flex;gap:.75rem;\
                 align-items:baseline;padding:.4rem 1rem;background:#fef3c7;color:#4b3a05;\
                 font:500 .8125rem/1.4 system-ui,sans-serif;border-bottom:1px solid #d7b95c";
    let tekst = "flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap";
    format!(
        r#"<div data-poc-portaal style="{stijl}" title="{uitleg} {voorbehoud}"><strong style="flex:none">{titel} — demonstratie, {status}</strong>"#,
        stijl = stijl,
        titel = esc(poc.titel_achter_de_poort()),
        status = esc(poc.status.label()),
        uitleg = esc(poc.status.uitleg()),
        voorbehoud = esc(poc.voorbehoud.trim()),
    ) + &format!(
        r#"<span style="{tekst}">{voorbehoud}</span><a href="/" style="flex:none;color:inherit">Alle proof-of-concepts</a></div>"#,
        tekst = tekst,
        voorbehoud = esc(poc.voorbehoud.trim()),
    )
}

/// The login screen for one PoC.
///
/// Served with 401, not a redirect: a deep link keeps its address, so signing
/// in lands the visitor where they were going instead of on the index.
///
/// Public, and reachable by guessing a slug, so it shows the same public title
/// and summary as the card and not a word more. It used to carry the internal
/// summary plus the full voorbehoud — which named the dossier, the department
/// and the bill — to anyone who typed the path.
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
      <nldd-link href="/" size="md" start-icon="arrow-left"
        text="Terug naar het overzicht"></nldd-link>
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

    /// Both pages open with the design system's required root.
    ///
    /// Not a style rule: `nldd-app-view` carries `min-height: 100dvh`, and
    /// `nldd-page` inside it only has `height: 100%`. Without the wrapper the
    /// page background ends wherever the content ends and the rest of the
    /// viewport is bare — which is exactly how this shipped the first time.
    #[test]
    fn every_page_is_wrapped_in_the_app_view_root() {
        let r = registry();
        for html in [
            index(&r),
            inloggen(r.get("napp").expect("napp"), "/napp/", false),
        ] {
            let app_view = html.find("<nldd-app-view").expect("app-view is the root");
            let page = html.find("<nldd-page").expect("page");
            assert!(app_view < page, "nldd-page must sit inside nldd-app-view");
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

    /// Neither public page may carry anything the register marks as internal.
    ///
    /// Both are readable without a password — the login screen by guessing a
    /// slug — so a `titel_intern` or `voorbehoud` that reaches either one names
    /// the dossier to whoever walks past. That is how this shipped the first
    /// time: the login screen carried the internal summary and the full
    /// voorbehoud, which between them named the department, the internal
    /// document and the bill.
    ///
    /// Checked against the real register rather than a fixture, because the
    /// thing worth protecting is the text that is actually published.
    #[test]
    fn the_public_pages_carry_nothing_the_register_marks_internal() {
        let r = registry();
        for poc in &r.pocs {
            let publiek = [
                index(&r),
                inloggen(poc, &format!("/{}/", poc.slug), false),
                inloggen(poc, &format!("/{}/", poc.slug), true),
            ];
            for html in publiek {
                for (veld, tekst) in [
                    ("titel_intern", poc.titel_intern.as_deref()),
                    ("samenvatting_intern", poc.samenvatting_intern.as_deref()),
                    ("voorbehoud", Some(poc.voorbehoud.as_str())),
                ] {
                    let Some(tekst) = tekst else { continue };
                    assert!(
                        !html.contains(tekst.trim()),
                        "{} of {} reached a public page",
                        veld,
                        poc.slug,
                    );
                }
                // The status and the subject tags are checked as rendered tags
                // rather than as bare words. Both words occur innocently: a
                // public summary may open with "Een verkenning…", and a slug
                // like `nieuwkomersbekostiging` contains the tag "bekostiging".
                // What must not appear is the tag itself, and there is no other
                // reason for this markup to be on a public page.
                assert!(
                    !html.contains("<nldd-tag"),
                    "a tag on a public page of {} — status and subject tags are internal",
                    poc.slug,
                );
            }
        }
    }

    /// The internal title is what a visitor sees once they are in.
    #[test]
    fn the_strip_behind_the_gate_names_the_real_subject() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = voorbehoud_strip(poc);
        assert!(html.contains(poc.titel_achter_de_poort()));
        assert!(html.contains(poc.voorbehoud.trim()));
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
    ///
    /// `nldd-app-view` is in the list for a second reason: it is the required
    /// root, and dropping it is not a missing decoration but a page whose
    /// background stops halfway down the viewport.
    #[test]
    fn only_elements_that_exist_in_the_design_system_are_used() {
        const BESTAAT: &[&str] = &[
            "nldd-app-view",
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
            titel_intern: Some(r#"Intern <i>onderwerp</i>"#.into()),
            samenvatting_intern: Some("intern & geheim".into()),
            soort: crate::registry::Soort::Statisch,
            status: crate::registry::Status::Verkenning,
            voorbehoud: r#"Een "demo" & niets meer."#.into(),
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
