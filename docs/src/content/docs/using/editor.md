---
title: "Working in the Editor"
description: "For jurists and traject members: signing in, opening a law, working in a traject, editing an article and its machine-readable part, running scenarios, notes, and reviewing AI proposals."
---

This guide is for someone who works on laws in the editor at `editor.regelrecht.rijks.app`: a jurist checking an interpretation, or a member of a traject changing one. It follows the order in which you meet things. How the editor is built is on the component page [Editor](/components/frontend); the ideas behind trajects, scenarios and notes are in [Scenarios](/concepts/scenarios) and [Notes and Annotations](/concepts/notes-and-annotations).

The editor's interface is Dutch. Labels are quoted below as they appear on screen.

## Sign in

Reading the published corpus needs no account. Changing anything does.

1. Open the "Account" menu (the person icon in the top bar) and choose "Inloggen". You sign in with your RegelRecht account.
2. If you do not have an account, choose "Account aanvragen". Accounts are handed out to a small group of government jurists who are in touch with the team; the page explains how to ask by e-mail.
3. To sign out, choose "Log uit" in the same menu.

When your session expires, the editor shows "Je bent uitgelogd omdat je sessie is verlopen." Click "Opnieuw inloggen" to return to where you were.

What you may do depends on your role: `editor-reader` can read, `editor-writer` can edit laws and scenarios, `editor-admin` can also change settings for everyone. A role you were just given takes effect the next time you sign in. See [Authentication & Roles](/auth-and-roles) for the full model.

## Open a law

1. On "Home", type in the search field "Wet- en regelgeving zoeken" (on a small screen, click "Zoeken"). Type at least three characters.
2. Click a result. The law opens with its articles in the sidebar, starting with "Algemeen" for the law's own details.
3. Click an article. It opens in three read-only views: "Tekst", "Machine" (the machine-readable part) and "YAML".

If the law is not in the corpus and you are signed in, the search also looks on wetten.overheid.nl. Clicking such a result asks for the law to be harvested; the status goes from "Harvest aangevraagd" through "Wordt opgehaald..." to "Beschikbaar", after which a click opens the law.

Click the star ("Voeg toe aan favorieten") to keep a law under "Favorieten" in the sidebar. Laws you opened recently appear under "Recent bekeken".

## Work in a traject

You never edit the published corpus directly. Every change happens in a traject: a working copy with its own branch, shared with the people you invite. The active traject is part of the address (`/trajecten/<name>-<code>/…`), so a link you send opens in the same traject.

### Pick or start a traject

1. Click "Editor" in the top bar, or "Bewerken" on an article. If no traject is active yet, the list "Trajecten" opens.
2. Click an existing traject, or "Nieuw traject" at the bottom.
3. For a new traject, fill in "Naam" (required, for example "Tariefswijziging 2026") and optionally "Beschrijving".
4. Leave "Eigen GitHub-repo (i.p.v. standaard MinBZK-repo)" off unless your organization keeps its own corpus repository. With it off, the traject lives in a public repository, so do not put anything confidential in it. With it on, fill in "Repo owner", "Repo" and "Base branch". See [Private-repo trajects](/operations/private-repo-trajects).
5. Click "Maak traject aan". You land in the editor of the new traject.

To switch trajects later, click the traject name in the top bar and pick another under "Trajecten". "Corpus juris" in that list takes you back to the published corpus.

### Add a law to the traject

A traject starts empty. Laws you open or change there appear under "In dit traject".

1. Click the "+" button ("Nieuw") and choose "Wet toevoegen…".
2. Under "Zoeken", search by name, law id or BWB id.
3. For a law already in the corpus, click "Toevoegen aan traject". For a law that only exists on wetten.overheid.nl, click "Ophalen naar traject"; the request then appears under "Taken".

Under "Uploaden" you can instead offer a document (PDF or Word) that is converted into a law. The result comes back as a task to review.

### Invite members

Only a "Beheerder" of the traject can invite.

1. Open the traject menu and choose "Instellingen", then "Leden". Or use "+" and "Leden uitnodigen…".
2. Click "Uitnodigen".
3. Enter one or more e-mail addresses under "E-mailadressen", separated by commas.
4. Choose a "Rol": "Bijdrager" (view and edit laws and scenarios) or "Beheerder" (also manage members and settings).
5. Click "Nodig uit".

Invited people show as "Openstaande uitnodiging" until they sign in for the first time. "Intrekken" withdraws an invitation. In the "Meer acties" menu next to a member you can change the role or choose "Verwijder lid".

## Edit an article

Open the article in the traject. The editor shows panes side by side: "Tekst", "Machine", "Scenario's", "YAML" and "Notities". Each pane has a "Weergave" menu to change what it shows and to move it left or right.

### The legal text

Edit the text in the "Tekst" pane directly. The toolbar offers bold and italic ("Tekststijl"), lists ("Lijst") and indentation.

### The machine-readable part

The "Machine" pane shows the article's logic in sections: "Definities", "Parameters", "Inputs", "Outputs" and "Acties".

1. To add something, click the button under its section, for example "Parameter toevoegen".
2. To change or remove a row, open its menu and choose "Bewerk" or "Verwijder".
3. In the sheet that opens, fill in the fields (such as "Naam", "Type", "Waarde", or "Bron regelgeving" and "Bron output" for a value that comes from another law) and click "Opslaan".

The "Opslaan" in the sheet for a definition, parameter, input or output only changes the article in your screen; it is written with the save described below. The sheet for an "Actie" saves the law straight away.

If an article has no machine-readable part yet, the pane says "Geen machine-leesbare gegevens voor dit artikel" and offers two ways forward:

- "Verrijk deze wet" asks an AI model to write a proposal for the whole law. While it works, the pane says "We genereren een voorstel". When it is done, "Beoordeel voorstel" takes you to the review (see [Taken](#review-ai-proposals-under-taken)).
- "Stel handmatig op" lets you write it yourself.

The "YAML" pane shows the whole law file and can be edited there too. It shows "YAML parse error" when the text is not valid YAML, and "De engine kan deze wet niet laden" when the engine rejects the result.

### Save

As soon as the article has unsaved changes, a bar appears with "Ongedaan maken", "Opnieuw" and "Opslaan".

1. Click "Opslaan".
2. The editor writes the law as a commit to the traject's branch. The first save also opens a pull request for the traject; later saves add to it.
3. A "PR #…" button appears in the top bar. It opens the pull request, where the change can be reviewed before it reaches the corpus.

If someone else changed the same law in the meantime, a dialog "Opslaan mislukt" says "De wet is intussen door iemand anders gewijzigd." Reload the page, redo your change, and save again. Nothing of theirs is overwritten.

## Run a scenario

A scenario is a worked example for the law: given these facts, the law should produce this outcome. The "Scenario's" pane lists the scenarios for the law, with their "Verwachte uitkomsten". If the law has several scenario files, pick one from the dropdown.

1. Click "Resultaat" on a scenario. It runs in your browser.
2. The sheet "Resultaat: …" shows each expected value next to the actual one, under "Verwacht" and "Uitkomst", with "Geslaagd" or "Mislukt" per line.
3. Below that, "Execution trace" shows every step the engine took, including the values it fetched from other laws. That is where to look when a result differs from what you expected.
4. "Graaf" shows the same run as a graph of the laws involved. Step through it with "◀ Vorige" and "Volgende ▶".

To try other facts, click "Bewerk". Change the values under "Invoer"; the scenario reruns as you type. Click "Opslaan" to keep the change, or "Opslaan en toon resultaat" to keep it and see the result.

The editor has no button to create a new scenario; new scenarios are added as files in the traject.

## Add a note

A note ties a remark to a specific passage of the legal text.

1. In the "Tekst" pane, select the passage.
2. Click "Notitie toevoegen" in the toolbar.
3. Write the remark under "Opmerking". Optionally add "Labels" and link it to an element or document under "Koppel aan".
4. Switch on "Taak voor mezelf" to keep it as a to-do.
5. Click "Voeg notitie toe".

If the editor warns "Te algemeen" or "Niet eenduidig", select a longer passage, so the note can be found again when the text changes.

A new note is private and stays in your browser. To share it with the traject, choose "Delen" on the note and confirm with "Deel binnen traject". Sharing writes it to the traject branch and cannot be undone. Notes are highlighted in the text and collected in the "Notities" pane.

## Review AI proposals under "Taken"

When an AI model enriches a law or converts a document, its output does not go into the law by itself. It becomes tasks for you to review.

1. Open the traject menu and choose "Taken". A red count on "Taken" means something failed.
2. Pick a category: "Prioriteit" (failed jobs), "Wachten op" (jobs still running), "Alle taken", or a single law.
3. On a task such as "Beoordeel artikel 3 van …", open "Acties" and choose "Beoordelen". The article opens in the editor with the proposal filled in.
4. The banner reads "Dit is een gegenereerd voorstel." and says which panes changed. Check the "Machine" and "YAML" panes, and run the scenarios.
5. Click "Neem voorstel over" to accept this part, or "Verwerp voorstel" to reject it. You may edit the proposal before accepting.
6. Continue with the next part. The banner shows "Onderdeel N van M".

Nothing is written until every part of the enrichment has a verdict. Then all accepted parts go into one commit on the traject branch. To stop early, "Rond af, neem de rest niet over" closes the enrichment and discards the parts you have not reviewed.

A failed task offers "Toon details" and "Probeer opnieuw". "Markeer als gedaan" removes a task from your list without a verdict.

## Werkdocumenten

A traject can also hold working documents, such as a memo or a draft explanatory note. They live on the same branch as the laws.

1. Choose "Werkdocumenten" in the traject menu, or "+" and "Nieuw document" or "Document uploaden…".
2. Edit the document and click "Opslaan".

When uploading, "Omzetten met AI toestaan" lets an AI model convert the document; the result comes back as a task.

## Settings that change what you see

Some parts of the editor can be switched off for a whole deployment. If a pane is missing, that is usually why.

| Switch in "Instellingen" → "Beheer" | What it hides or requires |
|---|---|
| "Tekst editor", "Machine editor", "Scenario editor", "YAML editor", "Notities" | The pane of that name. All are on by default |
| "Met eigen GitHub-account schrijven" | When on, saving needs a linked GitHub account. Off by default |

Only an `editor-admin` sees "Beheer", and a change there applies to every user. When the GitHub switch is on, other users link their account under "Instellingen" → "Koppelingen" with "Koppelen".

The color scheme is a personal choice: "Weergave" in the account menu, with "Systeem", "Licht" and "Donker".
