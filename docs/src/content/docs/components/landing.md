# Landing Page

The landing page is the public-facing website at `regelrecht.rijks.app`.

## Overview

- **Technology**: Static HTML / CSS / JS
- **Location**: `landing/`
- **Production URL**: `regelrecht.rijks.app`
- **Language**: Dutch

## What it does

A static website aimed at policy makers and civil servants. It explains what RegelRecht is, how it works, shows the ecosystem of tools, and provides examples of machine-readable law in action. Uses the `@nl-rvo` (Rijksoverheid) design system.

Sections: introduction, how it works, tools overview, examples, FAQ, and a contact form.

## Running locally

No build step. Serve with any static file server, or build and run the Docker image:

```bash
docker build -t regelrecht-landing -f landing/Dockerfile .
docker run -p 8000:8000 regelrecht-landing
```

## Further reading

- [Deployment](/operations/deployment) - how the landing page is deployed
