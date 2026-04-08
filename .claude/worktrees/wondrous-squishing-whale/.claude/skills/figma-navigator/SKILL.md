---
name: figma-navigator
description: Provides context about the regelrecht Figma design system and helps with Figma-related tasks like component lookups and design comparisons. Use this skill proactively when: implementing UI components, looking up Figma node IDs, comparing code with Figma designs, downloading Figma assets, or working with design tokens. Activate automatically when user mentions Figma, design system, component styling, or asks about RR-Components.
allowed-tools: Read, mcp__figma-with-token__get_figma_data, mcp__figma-with-token__download_figma_images
---

# Figma Navigator

This skill provides quick access to the RegelRecht (RR) Figma design system structure and helps with Figma-related tasks.

## Figma File Info

- **File Key:** `5DyHMXUNVxbgH7ZjhQxPZe`
- **File Name:** RR-Components
- **URL:** https://www.figma.com/design/5DyHMXUNVxbgH7ZjhQxPZe/RR-Components

## Quick Lookup

For component IDs and structure, read the reference file:
```
.claude/skills/figma-navigator/reference.md
```

This contains all component node IDs, pages, variants, and implementation details.

## Workflows

### 1. Find a Component's Figma ID

1. Check `reference.md` for the component
2. Use the node ID directly with the Figma MCP tool

Example:
```
mcp__figma-with-token__get_figma_data(fileKey: "5DyHMXUNVxbgH7ZjhQxPZe", nodeId: "20-27")
```

### 2. Get Component Details

When you need current properties, variants, or layout info:

```
mcp__figma-with-token__get_figma_data(
  fileKey: "5DyHMXUNVxbgH7ZjhQxPZe",
  nodeId: "<component-node-id>"
)
```

The MCP tool returns:
- Component variants and their node IDs
- Layout properties (padding, gap, sizing)
- Text styles and colors
- Border radius and strokes

### 3. Compare Figma vs Implementation

1. Get component specs from Figma using node ID from `reference.md`
2. Read the implementation file (listed in reference.md)
3. Compare:
   - Token usage (should match Figma variables)
   - Sizing (min-height, padding, gap)
   - Variants (all Figma variants should be implemented)
   - States (disabled, focused, hover)

### 4. Download Component Images

For pixel comparison or assets:

```
mcp__figma-with-token__download_figma_images(
  fileKey: "5DyHMXUNVxbgH7ZjhQxPZe",
  nodes: [
    { nodeId: "<node-id>", fileName: "component-name.png" }
  ],
  localPath: "/path/to/save"
)
```

### 5. Find Specific Variant

Each component set contains multiple variants. The variant node IDs follow patterns:

- **Button variants:** Inside component set `20:27`
  - Specific variant: `style=accent-filled, size=m` = `20:28`
  - Pattern: `style={style}, size={size}, is-disabled={bool}`

Use `get_figma_data` on the component set to see all variants.

## Rate Limits

The Figma MCP can hit rate limits. If you get errors:
1. Wait 60-120 seconds
2. Use cached info from `reference.md` when possible
3. Open Figma in browser as fallback

## Token Hierarchy

The design system uses three token layers:

```
Primitives (--primitives-*)  → base values (colors, space, opacity)
Semantics (--semantics-*)    → meaningful tokens (buttons, controls, focus)
Components (--components-*)  → component-specific tokens
```

Always use semantic tokens when available. Only use primitives when no semantic exists.

## Key Design Tokens

### Controls (sizing)
- `--semantics-controls-xs-min-size: 24px`
- `--semantics-controls-s-min-size: 32px`
- `--semantics-controls-m-min-size: 44px`

### Focus Ring
- `--semantics-focus-ring-thickness: 2px`
- `--semantics-focus-ring-color: #0f172a`

### Disabled State
- Use `calc(var(--primitives-opacity-disabled, 38) / 100)` for opacity

## Tips

1. **Start with reference.md** - Don't search Figma from scratch
2. **Use node IDs with colons** - Format is `123:456` or `123-456` (both work)
3. **Component sets vs components** - Component sets (like `20:27`) contain multiple variant components
4. **Check asymmetric padding** - Figma sometimes uses different top/bottom padding

---

## Storybook Integration

The project uses **@minbzk/storybook** web components. When implementing UI, always use these components instead of custom HTML/CSS.

**Storybook Docs:** https://minbzk.github.io/storybook

### Available Web Components

| Component | Tag | Variants |
|-----------|-----|----------|
| Box | `<rr-box>` | padding, radius |
| Button | `<rr-button>` | accent-filled, accent-outlined, accent-tinted, neutral-tinted, accent-transparent, danger-tinted; sizes: xs, s, m |
| Checkbox | `<rr-checkbox>` | checked, indeterminate, disabled |
| Icon Button | `<rr-icon-button>` | Same variants as Button |
| Menu Bar | `<rr-menu-bar>` | With title, links, disabled items |
| Radio Button | `<rr-radio-button>` | In groups |
| Switch | `<rr-switch>` | On/off states, disabled |
| Toggle Button | `<rr-toggle-button>` | selected states, with icons |
| Top Navigation Bar | `<rr-top-navigation-bar>` | With logo, back button, utility menu |
| Back Button | `<rr-back-button>` | Sub-component |
| Logo | `<rr-logo>` | Sub-component with branding |
| Utility Menu Bar | `<rr-utility-menu-bar>` | Search, help, settings |

### Figma → Storybook Mapping

When converting Figma designs to code:

| Figma Component | Storybook Tag |
|-----------------|---------------|
| button (accent-filled) | `<rr-button variant="accent-filled">` |
| button (accent-outlined) | `<rr-button variant="accent-outlined">` |
| icon-button | `<rr-icon-button>` |
| checkbox-list-cell | `<rr-checkbox>` |
| radio-button-list-cell | `<rr-radio-button>` |
| switch-list-cell | `<rr-switch>` |
| toggle-button | `<rr-toggle-button>` |
| top-navigation-bar | `<rr-top-navigation-bar>` |

### Components NOT in Storybook

These components don't exist in Storybook and require custom CSS:

| Component | Description | Custom CSS Location |
|-----------|-------------|---------------------|
| List | Lists with sections, headers, chevrons | `css/components/list.css` |
| Tabs | Tab navigation with panels | `css/components/tabs.css` |
| Search Field | Input with search icon | Custom |
| Split Pane Layout | Multi-column layout system | `css/layout.css` |
| Editor Toolbar | Rich text toolbar (B, I, U, undo, redo) | `css/components/editor.css` |

### Usage in HTML

```html
<!-- Import web components -->
<script type="module" src="/main.js"></script>

<!-- Use components -->
<rr-button variant="accent-filled" size="m">Primary Button</rr-button>
<rr-button variant="accent-outlined" size="m">Secondary Button</rr-button>
<rr-icon-button variant="neutral-tinted" size="s">
  <svg>...</svg>
</rr-icon-button>
```

### Installation

```bash
npm install @minbzk/storybook
```

Requires `GITHUB_TOKEN` with `read:packages` scope for npm registry access.
