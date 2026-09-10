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

1. **Create the file.** `just note "Titel van de notitie"` writes
   `docs/src/content/notes/YYYY-MM-DD-slug.md` with the frontmatter filled in
   and prints the URL it will be published at. Doing it by hand works too: the
   filename sets the URL, so `2026-09-10-voorbeeldnotitie.md` becomes
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

   The scaffolded file carries one comment line — *wat hadden we mis?* — as a
   prompt rather than a section. Answering it is usually what makes a note
   worth reading; deleting it is fine too.

4. **Open a pull request.** `just notes` previews it locally at
   <http://localhost:4321/notes>. Merging to `main` publishes it.

## Without a local checkout

You do not need git, a terminal or a clone. This link opens GitHub's file
editor with the frontmatter already filled in:

[**Write a note in the browser**](https://github.com/MinBZK/regelrecht/new/main?filename=docs%2Fsrc%2Fcontent%2Fnotes%2FJJJJ-MM-DD-slug.md&value=---%0Atitle%3A%20Titel%20van%20de%20notitie%0Adate%3A%20%27JJJJ-MM-DD%27%0Aauthors%3A%0A%20%20-%20name%3A%20Je%20naam%0A%20%20%20%20role%3A%20je%20rol%0Asummary%3A%20%3E-%0A%20%20Een%20of%20twee%20zinnen.%20Dit%20is%20wat%20het%20overzicht%20en%20de%20RSS-feed%20tonen.%0Atags%3A%20%5B%5D%0A---%0A%0A%3C%21--%20Wat%20hadden%20we%20mis%3F%20Dat%20stuk%20wordt%20het%20vaakst%20gelezen.%20Regel%20mag%20weg.%20--%3E%0A%0ASchrijf%20hier.%0A)

Replace `JJJJ-MM-DD` in the filename and in `date` with the date, replace
`slug` with a few words from the title, write the text, and press **Commit
changes**. GitHub creates the branch and the pull request for you.

If you cannot reach the repository at all, send the text to someone who can.
A note under a role without a name is fine, so it can be published as it
arrived.

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
