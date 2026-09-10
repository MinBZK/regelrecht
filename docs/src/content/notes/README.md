# Notes

Notes published at <https://regelrecht.rijks.app/notes>. One markdown file per
note, in this directory. They are built by the same Astro pipeline as the rest
of the site — there is no CMS.

Notes are written in Dutch. This README, like the rest of the repository
documentation, is in English; the writing guidelines below are quoted in the
language they were given in.

Not to be confused with the stand-off notes of RFC-005 and RFC-018, which are
annotations on legal text inside the editor. These are site pages.

## Adding a note, in four steps

1. **Create the file.** `docs/src/content/notes/YYYY-MM-DD-slug.md`. The filename
   sets the URL: `2026-09-10-voorbeeldnotitie.md` is published at
   `/notes/2026/09/voorbeeldnotitie`. Pick the slug once — the URL is a
   contract and will not be changed afterwards.

2. **Fill in the frontmatter.**

   ```yaml
   ---
   title: 'Title of the note'
   date: '2026-09-10'          # must match the date in the filename
   authors:
     - name: Your Name         # optional — a role on its own is fine
       role: your role
   summary: >-
     One or two sentences. This is what the overview page and the RSS feed show.
   tags:                       # optional
     - a-tag
   regulations:                # optional, law $id from corpus/regulation
     - wet_op_de_zorgtoeslag
   ---
   ```

   `regulations` links the note through to each law in the public reading
   environment. A typo fails the build.

3. **Write it.** Plain markdown. Start headings at `##` — the title is already
   the page's `h1`. There is no template and there are no required sections.

4. **Open a pull request.** `just notes` previews it locally at
   <http://localhost:4321/notes>. Merging to `main` publishes it.

## Writing guidelines

Dit zijn defaults, geen regels. Je mag ervan afwijken als de post erom vraagt;
je hoeft dat niet te verantwoorden.

Bij het schrijven:

- Eén idee per post
- Ik-vorm, met naam en rol van de auteur erbij

Voor publicatie, vier vinkjes:

- Is elke afkorting bij eerste gebruik uitgeschreven?
- Werken de links?
- Is er iemand die dit had moeten zien voor publicatie? (meestal: nee)
