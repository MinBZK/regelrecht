# RegelRecht UI Prototype

Static HTML/CSS/JS prototype for the RegelRecht user interface.

## Prerequisites

- Node.js 18+

## Setup

```bash
cd frontend
npm install
```

## Development

```bash
# Start development server with hot reload
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Browser Support

This prototype uses modern CSS features including:
- CSS `:has()` selector (Chrome 105+, Safari 15.4+, Firefox 121+)
- CSS custom properties (variables)
- CSS Grid and Flexbox

## Project Structure

```
frontend/
├── assets/
│   ├── icons/          # SVG icons
│   └── rijkswapen.svg  # National emblem
├── css/
│   ├── components/     # Component-specific styles
│   ├── layout.css      # Page layout styles
│   ├── main.css        # CSS entry point
│   ├── reset.css       # CSS reset
│   └── variables.css   # Design tokens
├── fonts/              # Rijksoverheid fonts
├── index.html          # Single-page app entry point
└── src/
    ├── main.js         # Vue app bootstrap + router
    ├── router.js       # Vue Router (Library + Editor routes)
    ├── LibraryApp.vue  # Library view (/library)
    └── EditorApp.vue   # Editor view (/editor/:lawId?)
```

## Components

### From @nldd/design-system
- `<rvo-button>` - Buttons
- `<rvo-navbar>` - Navigation bar
- `<rvo-toggle-button>` - Toggle buttons

### Custom CSS Components
- Lists with collapsible items
- Tab navigation
- Split pane layouts
