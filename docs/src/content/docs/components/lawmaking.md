# Lawmaking Frontend

The lawmaking frontend visualizes the Dutch legislative process as an interactive flow diagram.

## Overview

- **Language**: Vue 3 / Vite
- **Location**: `frontend-lawmaking/`
- **Production URL**: `lawmaking.regelrecht.rijks.app`

## What it does

The app presents the legislative process as an animated, step-through diagram. Users can advance through stages (from concept to publication), zoom in on details, and switch between three views:

- **Simple** - the basic legislative path
- **Advanced** - detailed stages and decision points
- **Wet open overheid (WOO)** - the process through the lens of open government law

No backend is needed. Flow data is defined as static JavaScript modules in `src/data/`.

## Running locally

```bash
cd frontend-lawmaking
npm install
npm run dev
```

## Further reading

- [Deployment](/operations/deployment) - how the lawmaking site is deployed
