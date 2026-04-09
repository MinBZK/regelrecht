---
name: page-hierarchy
description: Activeer bij het samenstellen van NDD Storybook web components tot pagina's en layouts — beschrijft de verplichte component-hiërarchie, nesting-regels en beschikbare layout-patronen
user-invocable: true
argument-hint: <layout-type>
---

Gebruik deze skill wanneer je NDD Storybook web components gaat samenstellen tot pagina's, views of layouts. Hij beschrijft welke componenten in welke volgorde genest moeten worden.

Context: $ARGUMENTS

## Overzicht

Elke NDD applicatie volgt een vaste hiërarchie van layout-componenten. De buitenste laag is altijd `ndd-app-view`, de binnenste laag bevat content-componenten.

```
ndd-app-view                              ← verplichte root
  └── Split View OF ndd-page              ← layout keuze
      └── ndd-page                        ← pagina met header/footer
          ├── slot="header"               ← navigatie, titelbalk
          ├── Secties                     ← content layout
          │   └── Content-componenten     ← tekst, lijsten, formulieren
          └── slot="footer"               ← footer content
```

---

## Laag 1: App View (verplicht)

```html
<ndd-app-view background="default|tinted">
  <!-- Eén split view OF één ndd-page -->
</ndd-app-view>
```

| Attribuut | Waarden | Beschrijving |
|-----------|---------|--------------|
| `background` | `default`, `tinted` | Cascade van `--context-parent-background-color` naar alle afstammelingen |

**Regel:** `ndd-app-view` is altijd de root. Bevat exact één direct child: een split view of een `ndd-page`.

---

## Laag 2: Layout keuze

Kies één layout-type op basis van de applicatie:

### Optie A: Navigation Split View (meest gebruikt)

Vier-koloms layout met sidebar, secundaire sidebar, main content en inspector. Panelen verschijnen automatisch wanneer content geslot wordt.

```html
<ndd-navigation-split-view>
  <ndd-page slot="sidebar">...</ndd-page>
  <ndd-page slot="secondary-sidebar">...</ndd-page>
  <ndd-page slot="main">...</ndd-page>
  <ndd-page slot="inspector">...</ndd-page>
</ndd-navigation-split-view>
```

| Slot | Beschrijving | Verplicht |
|------|--------------|-----------|
| `sidebar` | Primaire navigatie (links) | Nee, maar typisch aanwezig |
| `secondary-sidebar` | Subnavigatie (tweede kolom) | Nee |
| `main` | Primaire inhoud | Ja |
| `inspector` | Details/eigenschappen (rechts) | Nee |

**Responsief gedrag:** Panelen die niet passen worden automatisch verborgen en beschikbaar als sheet (overlay).

| Attribuut | Beschrijving |
|-----------|--------------|
| `inspector-as-sheet` | Inspector altijd als sheet tonen |
| `sidebar-as-sheet` | Sidebar altijd als sheet, main op volle breedte |
| `inspector-accessible-label` | Toegankelijke naam voor inspector sheet |
| `sidebar-accessible-label` | Toegankelijke naam voor sidebar sheet |

**Methoden:** `showInspectorSheet()`, `hideInspectorSheet()`, `showSidebarSheet()`, `hideSidebarSheet()`

### Optie B: Bar Split View

Verticale layout met een main-gebied en onbeperkt aantal bars (toolbars, statusbalken). Bars kunnen per breakpoint geordend worden.

```html
<ndd-bar-split-view>
  <ndd-page slot="toolbar" sm-order="1" md-order="1">...</ndd-page>
  <ndd-page slot="main" sm-order="2" md-order="2">...</ndd-page>
  <ndd-page slot="status-bar" sm-order="3" md-order="3">...</ndd-page>
</ndd-bar-split-view>
```

| Attribuut (op children) | Beschrijving |
|-------------------------|--------------|
| `sm-order`, `md-order`, `lg-order` | Volgorde per breakpoint |
| `above="sm\|md\|lg"` | Toon vanaf dit breakpoint en groter |
| `below="sm\|md\|lg"` | Toon tot en met dit breakpoint |
| `only="sm\|md\|lg"` | Toon alleen op dit breakpoint |

**Responsief gedrag:** Op sm-viewports overlappen bars de main area. Bars vóór main stapelen top-to-bottom, bars ná main stapelen bottom-to-top.

### Optie C: Side by Side Split View

Horizontale gelijke panelen naast elkaar. Panelen die niet passen worden automatisch verborgen.

```html
<ndd-side-by-side-split-view panes="3">
  <ndd-page slot="pane-1">...</ndd-page>
  <ndd-page slot="pane-2">...</ndd-page>
  <ndd-page slot="pane-3">...</ndd-page>
</ndd-side-by-side-split-view>
```

### Optie D: Stacked Split View

Verticale gelijke panelen gestapeld. Panelen die niet passen worden automatisch verborgen.

```html
<ndd-stacked-split-view panes="2">
  <ndd-page slot="pane-1">...</ndd-page>
  <ndd-page slot="pane-2">...</ndd-page>
</ndd-stacked-split-view>
```

### Optie E: Enkele pagina (geen split view)

Voor eenvoudige pagina's zonder navigatiepanelen.

```html
<ndd-app-view>
  <ndd-page sticky-header>...</ndd-page>
</ndd-app-view>
```

---

## Laag 3: Pagina (ndd-page)

Elk paneel in een split view bevat een `ndd-page`. Een page biedt scrollgedrag, optionele sticky header en footer.

```html
<ndd-page sticky-header sticky-footer background="inherit|default|tinted">
  <ndd-container slot="header" padding="16">
    <!-- Navigatiebalk of titelbalk -->
  </ndd-container>

  <!-- Secties met content -->
  <ndd-simple-section>...</ndd-simple-section>

  <ndd-container slot="footer" padding="16">
    <!-- Footer content -->
  </ndd-container>
</ndd-page>
```

| Attribuut | Beschrijving |
|-----------|--------------|
| `sticky-header` | Header blijft bovenaan bij scrollen |
| `sticky-footer` | Footer blijft onderaan |
| `background` | `inherit` (van parent), `default` (wit), `tinted` (grijs) |

| Slot | Beschrijving |
|------|--------------|
| `header` | Navigatie, titelbalk (optioneel sticky) |
| default | Scrollbare content (secties) |
| `footer` | Footer content (optioneel sticky) |

---

## Laag 4: Header-componenten

### Top Navigation Bar (applicatie-niveau)

De hoofdnavigatiebalk met logo, titel, menu en utility-items. Typisch in de header van de sidebar of bovenste page.

```html
<ndd-top-navigation-bar
  title="Mijn Applicatie"
  container="md"
  logo-has-wordmark
  logo-title="Rijksoverheid"
>
</ndd-top-navigation-bar>
```

| Attribuut | Beschrijving |
|-----------|--------------|
| `title` | Paginatitel |
| `container` | `sm`, `md`, `lg` |
| `no-logo`, `no-title`, `no-menu` | Verberg onderdelen |
| `has-back-button`, `back-href`, `back-text` | Terugknop |
| `logo-has-wordmark`, `logo-title`, `logo-subtitle` | Logo met tekst |
| `utility-no-language-switch`, `utility-no-search`, `utility-no-account` | Verberg utility items |

### Top Title Bar (pagina/paneel-niveau)

Titelbalk voor panelen met optionele terugknop en toolbar. Wordt automatisch compact bij scrollen.

```html
<ndd-top-title-bar
  text="Documenttitel"
  supporting-text="Ondertitel"
  back-text="Terug naar overzicht"
  collapse-anchor="content-heading"
>
  <ndd-icon-button slot="toolbar" icon="edit"></ndd-icon-button>
</ndd-top-title-bar>
```

| Attribuut | Beschrijving |
|-----------|--------------|
| `text` | Titel |
| `supporting-text` | Ondertitel |
| `back-text`, `back-href` | Terugknop |
| `dismiss-text` | Sluitknop |
| `collapse-anchor` | ID van element dat compact-modus triggert bij scrollen |

| Slot | Beschrijving |
|------|--------------|
| `toolbar` | Actieknoppen naast de sluitknop |

### Tab Bar (navigatie binnen paneel)

```html
<ndd-tab-bar navigation responsive accessible-label="Hoofdnavigatie">
  <ndd-tab-bar-item text="Overzicht" href="/overzicht" selected>
    <ndd-icon slot="icon" name="home"></ndd-icon>
  </ndd-tab-bar-item>
  <ndd-tab-bar-item text="Instellingen" href="/instellingen">
    <ndd-icon slot="icon" name="settings"></ndd-icon>
  </ndd-tab-bar-item>
</ndd-tab-bar>
```

| Attribuut | Beschrijving |
|-----------|--------------|
| `navigation` | Rendert als `<nav>` in plaats van tablist (voor route-navigatie) |
| `responsive` | Automatisch compact onder 480px |
| `full-width` | Volle breedte |
| `compact` | Altijd icon boven tekst |
| `variant` | `icon-and-text`, `text`, `icon` |

### Menu Bar (horizontaal menu)

```html
<ndd-menu-bar has-overflow-menu>
  <ndd-menu-bar-item selected>Wetten</ndd-menu-bar-item>
  <ndd-menu-bar-item>Regelingen</ndd-menu-bar-item>
  <ndd-menu-bar-item>Besluiten</ndd-menu-bar-item>
</ndd-menu-bar>
```

| Attribuut | Beschrijving |
|-----------|--------------|
| `has-overflow-menu` | Automatische overflow-knop bij beperkte ruimte |
| `size` | `s`, `m`, `l` |

### Document Tab Bar (document-tabs)

```html
<ndd-document-tab-bar accessible-label="Open documenten">
  <ndd-document-tab-bar-item text="Document 1" selected></ndd-document-tab-bar-item>
  <ndd-document-tab-bar-item text="Document 2"></ndd-document-tab-bar-item>
  <ndd-icon-button slot="end" icon="plus"></ndd-icon-button>
</ndd-document-tab-bar>
```

---

## Laag 5: Secties (content layout)

Secties organiseren content binnen een page. Ze bieden responsieve padding en gap via container queries.

### Beschikbare secties

| Component | Layout | Beschrijving |
|-----------|--------|--------------|
| `ndd-simple-section` | Enkele kolom | Basis sectie met header/footer slots |
| `ndd-full-bleed-section` | Volle breedte | Zonder horizontale padding (achtergrondkleuren, afbeeldingen) |
| `ndd-one-third-two-thirds-section` | 1/3 + 2/3 | Sidebar links, content rechts |
| `ndd-two-thirds-one-third-section` | 2/3 + 1/3 | Content links, sidebar rechts |
| `ndd-one-half-one-half-section` | 1/2 + 1/2 | Twee gelijke kolommen |

### Voorbeeld: Simple Section

```html
<ndd-simple-section>
  <ndd-rich-text slot="header"><h2>Titel</h2></ndd-rich-text>
  <ndd-rich-text>
    <p>Inhoud van de sectie.</p>
  </ndd-rich-text>
  <ndd-rich-text slot="footer"><p>Voetnoot</p></ndd-rich-text>
</ndd-simple-section>
```

### Voorbeeld: Twee-koloms sectie

```html
<ndd-one-third-two-thirds-section>
  <ndd-rich-text slot="header"><h2>Titel</h2></ndd-rich-text>
  <ndd-rich-text slot="left">
    <p>Zijbalk content</p>
  </ndd-rich-text>
  <ndd-rich-text>
    <p>Hoofdinhoud (2/3 breedte)</p>
  </ndd-rich-text>
</ndd-one-third-two-thirds-section>
```

**Responsief:** Kolommen wrappen automatisch wanneer ze smaller worden dan 280px.

---

## Laag 6: Content-componenten

Binnen secties gebruik je content- en interactiecomponenten:

| Categorie | Componenten | Beschrijving |
|-----------|-------------|--------------|
| **Content** | `ndd-rich-text`, `ndd-title`, `ndd-icon`, `ndd-tooltip` | Tekst en visuele content |
| **Actions** | `ndd-button`, `ndd-icon-button`, `ndd-toolbar` | Knoppen en acties |
| **Inputs** | `ndd-text-field`, `ndd-dropdown`, `ndd-checkbox`, `ndd-radio-button`, `ndd-switch` | Formulier-invoer |
| **Forms** | `ndd-form-field` | Formulierveld wrapper met label en foutmelding |
| **Lists** | `ndd-list`, `ndd-menu`, `ndd-cell` | Lijsten en menu's |
| **Feedback** | `ndd-dialog`, `ndd-modal` | Dialogen en modals |
| **Layout** | `ndd-container`, `ndd-box`, `ndd-spacer`, `ndd-divider` | Spacing en groepering |

---

## Hulpcomponenten

### Container (padding wrapper)

```html
<ndd-container padding="16" md-padding="24" lg-padding="32">
  <!-- Content met responsieve padding -->
</ndd-container>
```

Geldige padding-waarden: `0`, `2`, `4`, `6`, `8`, `10`, `12`, `16`, `20`, `24`, `28`, `32`, `40`, `44`, `48`, `56`, `64`, `80`, `96`

### Box (visuele groepering)

```html
<ndd-box>
  <!-- Gerelateerde componenten in een visueel afgebakend gebied -->
</ndd-box>
```

### Sheet (overlay paneel)

```html
<ndd-sheet placement="right|left|bottom" accessible-label="Details">
  <!-- Sheet content -->
</ndd-sheet>
```

Methoden: `show()`, `hide()`

### Spacer

```html
<ndd-spacer size="32"></ndd-spacer>
```

Sizes: `2`, `4`, `6`, `8`, `12`, `16`, `20`, `24`, `32`, `40`, `44`, `48`, `64`, `80`, `96`, `m`, `flexible`

---

## Achtergrondkleur

Achtergrondkleur wordt gecascade via `--context-parent-background-color`:

1. Stel `background` in op `ndd-app-view` voor de hele applicatie
2. Of stel `background` in op individuele `ndd-page` componenten per paneel
3. Kinderen lezen de variabele automatisch

```html
<!-- Hele app tinted -->
<ndd-app-view background="tinted">...</ndd-app-view>

<!-- Per paneel -->
<ndd-app-view>
  <ndd-navigation-split-view>
    <ndd-page slot="sidebar" background="tinted">...</ndd-page>
    <ndd-page slot="main" background="default">...</ndd-page>
  </ndd-navigation-split-view>
</ndd-app-view>
```

---

## Voorbeeldpagina's

### Voorbeeld 1: Applicatie met navigatie

```html
<ndd-app-view background="default">
  <ndd-navigation-split-view>
    <!-- Sidebar met navigatie -->
    <ndd-page sticky-header slot="sidebar" background="tinted">
      <ndd-container slot="header" padding="16">
        <ndd-top-title-bar text="Navigatie"></ndd-top-title-bar>
      </ndd-container>
      <ndd-simple-section>
        <ndd-list>
          <!-- Navigatie-items -->
        </ndd-list>
      </ndd-simple-section>
    </ndd-page>

    <!-- Hoofdinhoud -->
    <ndd-page sticky-header slot="main">
      <ndd-container slot="header" padding="16">
        <ndd-top-title-bar text="Documenttitel" supporting-text="Laatst bewerkt: vandaag">
          <ndd-icon-button slot="toolbar" icon="edit"></ndd-icon-button>
        </ndd-top-title-bar>
      </ndd-container>
      <ndd-simple-section>
        <ndd-rich-text>
          <h2>Inhoud</h2>
          <p>Primaire content van de pagina.</p>
        </ndd-rich-text>
      </ndd-simple-section>
    </ndd-page>

    <!-- Inspector voor details -->
    <ndd-page sticky-header slot="inspector">
      <ndd-container slot="header" padding="16">
        <ndd-top-title-bar text="Eigenschappen"></ndd-top-title-bar>
      </ndd-container>
      <ndd-simple-section>
        <ndd-rich-text>
          <p>Details over het geselecteerde item.</p>
        </ndd-rich-text>
      </ndd-simple-section>
    </ndd-page>
  </ndd-navigation-split-view>
</ndd-app-view>
```

### Voorbeeld 2: Eenvoudige pagina

```html
<ndd-app-view>
  <ndd-page sticky-header>
    <ndd-top-navigation-bar
      slot="header"
      title="Rijksoverheid"
      logo-has-wordmark
      logo-title="Rijksoverheid"
    >
    </ndd-top-navigation-bar>

    <ndd-simple-section>
      <ndd-rich-text>
        <h1>Welkom</h1>
        <p>Een eenvoudige pagina zonder split view.</p>
      </ndd-rich-text>
    </ndd-simple-section>

    <ndd-two-thirds-one-third-section>
      <ndd-rich-text>
        <h2>Hoofdartikel</h2>
        <p>Content in 2/3 breedte.</p>
      </ndd-rich-text>
      <ndd-rich-text slot="right">
        <h3>Gerelateerd</h3>
        <p>Sidebar content in 1/3 breedte.</p>
      </ndd-rich-text>
    </ndd-two-thirds-one-third-section>

    <ndd-container slot="footer" padding="16">
      <ndd-rich-text>
        <p>Footer informatie</p>
      </ndd-rich-text>
    </ndd-container>
  </ndd-page>
</ndd-app-view>
```

### Voorbeeld 3: Applicatie met toolbar

```html
<ndd-app-view>
  <ndd-bar-split-view>
    <ndd-page slot="toolbar" sm-order="1" md-order="1">
      <ndd-tab-bar navigation responsive>
        <ndd-tab-bar-item text="Start" selected></ndd-tab-bar-item>
        <ndd-tab-bar-item text="Zoeken"></ndd-tab-bar-item>
      </ndd-tab-bar>
    </ndd-page>

    <ndd-page slot="main" sm-order="2" md-order="2" sticky-header>
      <ndd-container slot="header" padding="16">
        <ndd-top-title-bar text="Overzicht"></ndd-top-title-bar>
      </ndd-container>
      <ndd-simple-section>
        <ndd-rich-text>
          <p>Content onder de toolbar.</p>
        </ndd-rich-text>
      </ndd-simple-section>
    </ndd-page>
  </ndd-bar-split-view>
</ndd-app-view>
```

---

## Beslisboom: welke layout?

```
Heeft de app een zijnavigatie?
├── Ja → ndd-navigation-split-view
│   ├── Met details-paneel? → voeg inspector slot toe
│   └── Met subnavigatie? → voeg secondary-sidebar slot toe
│
├── Nee, maar wel toolbars/statusbalken?
│   └── ndd-bar-split-view
│
├── Nee, maar meerdere gelijke panelen naast elkaar?
│   └── ndd-side-by-side-split-view
│
├── Nee, maar meerdere gelijke panelen gestapeld?
│   └── ndd-stacked-split-view
│
└── Nee, gewoon één pagina
    └── ndd-page direct in ndd-app-view
```
