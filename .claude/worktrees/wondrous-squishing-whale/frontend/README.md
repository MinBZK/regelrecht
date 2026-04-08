# RegelRecht UI Prototype

Static HTML/CSS/JS prototype for the RegelRecht user interface.

## Prerequisites

- Node.js 18+
- GitHub Personal Access Token with `read:packages` scope

## Setup

### 1. Configure GitHub Token

This project uses `@minbzk/storybook` from GitHub Packages. You need to authenticate:

```bash
# Option 1: Set environment variable
export GITHUB_TOKEN=your_github_token_here

# Option 2: Login to GitHub npm registry
npm login --registry=https://npm.pkg.github.com
```

### 2. Install Dependencies

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
├── index.html          # Browser view
├── editor.html         # Editor view
└── main.js             # JavaScript entry point
```

## Components

### From @minbzk/storybook
- `<rvo-button>` - Buttons
- `<rvo-navbar>` - Navigation bar
- `<rvo-toggle-button>` - Toggle buttons

### Custom CSS Components
- Lists with collapsible items
- Tab navigation
- Split pane layouts
