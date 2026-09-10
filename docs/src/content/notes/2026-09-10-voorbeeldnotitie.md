---
title: 'Voorbeeldnotitie: zo ziet een notitie eruit'
date: '2026-09-10'
authors:
  - name: Voorbeeld Auteur
    role: ontwikkelaar
  - role: jurist
summary: >-
  Een voorbeeldnotitie met verzonnen inhoud, die alle velden uit de frontmatter
  gebruikt. Vervang of verwijder hem zodra er een echte notitie staat.
tags:
  - voorbeeld
  - werkwijze
regulations:
  - wet_op_de_zorgtoeslag
---

Deze notitie bestaat om te laten zien hoe een notitie eruitziet. De inhoud is
verzonnen. Wie de eerste echte schrijft, mag dit bestand weggooien.

Ik schrijf hier in de ik-vorm, omdat een notitie van iemand komt en niet van een
organisatie. Dat maakt ook makkelijker om iets te zeggen waar je nog niet
helemaal uit bent.

## Eén idee per notitie

Deze notitie gaat over één ding: hoe zo'n notitie is opgebouwd. Alles wat daar
niet bij hoort, hoort in een andere. Dat scheelt de lezer werk en het scheelt de
schrijver een structuur bedenken.

De opmaak is gewone markdown. Koppen beginnen op niveau twee, want de titel uit
de frontmatter is al de `h1` van de pagina. Verder is er niets verplicht: geen
vaste secties, geen sjabloon om in te vullen.

## Wat de frontmatter doet

De velden bovenaan dit bestand sturen vier dingen aan:

- `title`, `date` en `summary` vullen het overzicht.
- `authors` levert de regel onder de titel. De tweede auteur hierboven heeft
  geen naam — een rol alleen mag ook.
- `tags` staan onderaan. Er zijn geen tagpagina's; het is beschrijving, geen
  navigatie.
- `regulations` verwijst naar regelingen in het corpus. Deze notitie noemt de
  [Wet op de zorgtoeslag](https://wetten.overheid.nl/BWBR0018451), en onderaan
  staat automatisch een link naar diezelfde regeling in de leesomgeving.

De bestandsnaam bepaalt de URL. Dit bestand heet
`2026-09-10-voorbeeldnotitie.md` en staat dus op
`/notes/2026/09/voorbeeldnotitie`. Die URL verandert niet meer, ook niet als de
titel later wordt bijgeschaafd.
