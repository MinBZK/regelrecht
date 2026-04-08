# Figma Reference - RR-Components

## File Info

| Property | Value |
|----------|-------|
| File Key | `5DyHMXUNVxbgH7ZjhQxPZe` |
| File Name | RR-Components |
| URL | https://www.figma.com/design/5DyHMXUNVxbgH7ZjhQxPZe/RR-Components |

## Pages

| Page Name | Page ID | Description |
|-----------|---------|-------------|
| Lists | 0:1 | Buttons, checkboxes, radios, switches, icon buttons |
| Bars | - | Navigation bars, menu bars |
| Inputs and Selectors | - | Toggle buttons, form inputs |

## Component Quick Reference

| Component | Node ID | Page | Tag Name | Status |
|-----------|---------|------|----------|--------|
| Button | `20:27` | Lists | `rr-button` | Implemented |
| Checkbox | `236:41408` | Lists | `rr-checkbox` | Implemented |
| Radio Button | `236:41398` | Lists | `rr-radio` | Implemented |
| Switch | `236:41353` | Lists | `rr-switch` | Implemented |
| Toggle Button | `309:3542` | Inputs and Selectors | `rr-toggle-button` | Implemented |
| Icon Button | `240:1391` | Lists | `rr-icon-button` | Implemented |
| Menu Bar | `48:2135` | Bars | `rr-menu-bar` | Implemented |
| Top Navigation Bar | `48:2135` | Bars | - | Implemented |
| Box | - | - | `rr-box` | Utility (no Figma) |

---

## Component Details

### Button

**Node ID:** `20:27` (component set)
**Figma Name:** `button`
**Implementation:** `src/components/button/rr-button.js`

#### Properties
- `style`: accent-filled, accent-outlined, accent-tinted, neutral-tinted, accent-transparent, danger-tinted
- `size`: xs, s, m
- `is-disabled`: boolean
- `has-leading-icon`: boolean
- `has-trailing-icon`: boolean

#### Variant Node IDs (examples)
| Variant | Node ID |
|---------|---------|
| accent-filled, m, enabled | `20:28` |
| accent-outlined, m, enabled | `20:34` |
| accent-tinted, m, enabled | `48:1820` |
| accent-transparent, m, enabled | `20:40` |
| neutral-tinted, m, enabled | `306:861` |
| danger-tinted, m, enabled | `939:3042` |
| accent-filled, s, enabled | `20:46` |
| accent-filled, xs, enabled | `236:41792` |

#### Sizing
| Size | Min Height | Padding | Gap |
|------|------------|---------|-----|
| xs | 24px | 4px 6px | 2px |
| s | 32px | 6px 8px | 2px |
| m | 44px | 12px | 4px |

#### Tokens
- `--components-button-m-font`
- `--components-button-s-font`
- `--components-button-xs-font`
- `--semantics-buttons-accent-filled-background-color`
- `--semantics-buttons-accent-filled-color`

---

### Checkbox

**Node ID:** `236:41408` (component set)
**Figma Name:** `checkbox-list-cell`
**Implementation:** `src/components/checkbox/rr-checkbox.js`

#### Properties
- `size`: xs, s, m
- `is-checked`: boolean
- `is-indeterminate`: boolean
- `is-disabled`: boolean

#### Tokens
- `--components-checkbox-border-thickness`
- `--components-checkbox-border-color`
- `--components-checkbox-background-color`
- `--components-checkbox-is-selected-background-color`
- `--components-checkbox-is-selected-icon-color`

---

### Radio Button

**Node ID:** `236:41398` (component set)
**Figma Name:** `radio-button-list-cell`
**Implementation:** `src/components/radio/rr-radio.js`

#### Properties
- `size`: xs, s, m
- `is-checked`: boolean
- `is-disabled`: boolean

#### Tokens
- `--components-radio-button-border-thickness`
- `--components-radio-button-border-color`
- `--components-radio-button-background-color`
- `--components-radio-button-is-selected-background-color`

---

### Switch

**Node ID:** `236:41353` (component set)
**Figma Name:** `switch-list-cell`
**Implementation:** `src/components/switch/rr-switch.js`

#### Properties
- `size`: xs, s, m
- `is-checked`: boolean
- `is-disabled`: boolean

#### Tokens
- `--components-switch-border-thickness`
- `--components-switch-background-color`
- `--components-switch-thumb-background-color`
- `--components-switch-is-selected-background-color`

---

### Toggle Button

**Node ID:** `309:3542` (component set)
**Figma Name:** `toggle-button`
**Implementation:** `src/components/toggle-button/rr-toggle-button.js`

#### Properties
- `size`: xs, s, m
- `is-selected`: boolean
- `is-disabled`: boolean

#### Tokens
- `--components-toggle-button-content-color`
- `--components-toggle-button-background-color`
- `--components-toggle-button-is-selected-background-color`

---

### Icon Button

**Node ID:** `240:1391` (component set)
**Figma Name:** `icon-button-list-cell`
**Implementation:** `src/components/icon-button/rr-icon-button.js`

#### Properties
- `style`: accent-filled, accent-outlined, accent-tinted, neutral-tinted, accent-transparent
- `size`: xs, s, m
- `is-disabled`: boolean

#### Tokens
Same as Button, plus:
- `--components-icon-button-font`

---

### Menu Bar

**Node ID:** `48:2135` (component set)
**Figma Name:** `top-navigation-bar`
**Implementation:** `src/components/menu-bar/rr-menu-bar.js`

Note: Menu Bar is part of the Top Navigation Bar design in Figma.

#### Properties
- `size`: s, m, l
- `selected`: boolean
- `disabled`: boolean

#### Additional Components
- `rr-menu-item` (`src/components/menu-bar/rr-menu-item.js`)

#### Tokens
- `--components-menu-bar-menu-item-color`
- `--components-menu-bar-menu-item-font`
- `--components-menu-bar-menu-item-is-selected-indicator-color`
- `--components-menu-bar-title-item-s-font`
- `--components-menu-bar-title-item-m-font`
- `--components-menu-bar-title-item-l-font`

---

## Common Tokens

### Control Sizing
```css
--semantics-controls-xs-min-size: 24px
--semantics-controls-xs-corner-radius: 5px
--semantics-controls-s-min-size: 32px
--semantics-controls-s-corner-radius: 5px
--semantics-controls-m-min-size: 44px
--semantics-controls-m-corner-radius: 7px
```

### Focus Ring
```css
--semantics-focus-ring-thickness: 2px
--semantics-focus-ring-color: #0f172a
```

### Disabled State
```css
opacity: calc(var(--primitives-opacity-disabled, 38) / 100)
```

---

## Token Source

| Property | Value |
|----------|-------|
| File | `tokens/rr-tokens.json` |
| Plugin | variables2json v1.0.4 |
| Exported | 2024-12-20 |

---

## Usage Examples

### Get Button component data
```
mcp__figma-with-token__get_figma_data(
  fileKey: "5DyHMXUNVxbgH7ZjhQxPZe",
  nodeId: "20:27"
)
```

### Get specific Button variant
```
mcp__figma-with-token__get_figma_data(
  fileKey: "5DyHMXUNVxbgH7ZjhQxPZe",
  nodeId: "20:28"
)
```

### Download Button as PNG
```
mcp__figma-with-token__download_figma_images(
  fileKey: "5DyHMXUNVxbgH7ZjhQxPZe",
  nodes: [{ nodeId: "20:27", fileName: "button-all-variants.png" }],
  localPath: "/path/to/images"
)
```
