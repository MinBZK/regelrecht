import { describe, it, expect } from "vitest";
import {
  articleAnchor,
  groupArticles,
  findGroupForArticleNumber,
} from "./articleGroups.js";

const BASE = "https://wetten.overheid.nl/BWBR0018492/2026-01-01";
const a = (number, anchor) => ({
  number,
  text: `tekst ${number}`,
  url: anchor === null ? BASE : `${BASE}#${anchor}`,
});

describe("articleAnchor", () => {
  it("extracts the fragment after #", () => {
    expect(articleAnchor(`${BASE}#Artikel2.4`)).toBe("Artikel2.4");
  });
  it("returns null without a fragment", () => {
    expect(articleAnchor(BASE)).toBe(null);
    expect(articleAnchor(undefined)).toBe(null);
  });
});

describe("groupArticles — besluit_zorgverzekering shape", () => {
  // Over-split corpus: many leaf segments per article, all sharing an anchor.
  const articles = [
    a("Aanhef", "ArtikelAanhef"),
    a("1", "Artikel1"),
    a("1.a", "Artikel1"),
    a("1.e", "Artikel1"),
    a("1.e.1°", "Artikel1"),
    a("1.e.2°", "Artikel1"),
    a("1.f", "Artikel1"),
    a("2.1.1", "Artikel2.1"),
    a("2.1.2", "Artikel2.1"),
    a("2.4.1", "Artikel2.4"),
    a("2.4.1.a", "Artikel2.4"),
    a("2.4.1.a.1°", "Artikel2.4"),
  ];

  it("collapses leaf segments into one group per article", () => {
    const groups = groupArticles(articles);
    expect(groups.map((g) => g.number)).toEqual([
      "Aanhef",
      "1",
      "2.1",
      "2.4",
    ]);
  });

  it("does NOT show 1.e.1° as its own list item", () => {
    const groups = groupArticles(articles);
    expect(groups.some((g) => g.number === "1.e.1°")).toBe(false);
  });

  it("keeps all segments of an article, in YAML order", () => {
    const groups = groupArticles(articles);
    const art1 = groups.find((g) => g.number === "1");
    expect(art1.segments.map((s) => s.number)).toEqual([
      "1",
      "1.a",
      "1.e",
      "1.e.1°",
      "1.e.2°",
      "1.f",
    ]);
  });

  it("labels the preamble 'Aanhef' and numbered articles 'Artikel N'", () => {
    const groups = groupArticles(articles);
    expect(groups.find((g) => g.number === "Aanhef").label).toBe("Aanhef");
    expect(groups.find((g) => g.number === "2.4").label).toBe("Artikel 2.4");
  });

  it("keys anchor-grouped articles on the url fragment (drives list :selected)", () => {
    const groups = groupArticles(articles);
    // The list's selected-state compares group.key === selectedGroup.key, so
    // the key shape is part of the contract.
    expect(groups.find((g) => g.number === "2.4").key).toBe("#Artikel2.4");
    expect(groups.find((g) => g.number === "Aanhef").key).toBe("#ArtikelAanhef");
  });
});

describe("groupArticles — genuine dotted articles are not over-grouped", () => {
  // wet_inkomstenbelasting: each dotted number is a whole article with its own
  // anchor, so they must stay separate groups.
  const articles = [
    a("3.1", "Artikel3.1"),
    a("3.2", "Artikel3.2"),
    a("4.12", "Artikel4.12"),
  ];

  it("keeps each dotted article as its own group", () => {
    const groups = groupArticles(articles);
    expect(groups.map((g) => g.number)).toEqual(["3.1", "3.2", "4.12"]);
    expect(groups.every((g) => g.segments.length === 1)).toBe(true);
  });
});

describe("groupArticles — fragmentless entries stand alone", () => {
  it("groups entries without a url fragment by their own number", () => {
    const groups = groupArticles([a("1", null), a("2", null)]);
    expect(groups.map((g) => g.number)).toEqual(["1", "2"]);
  });
});

describe("findGroupForArticleNumber", () => {
  const groups = groupArticles([
    a("1", "Artikel1"),
    a("1.e.1°", "Artikel1"),
    a("2.4.1.a.1°", "Artikel2.4"),
  ]);

  it("matches by the group's representative number", () => {
    expect(findGroupForArticleNumber(groups, "2.4")?.number).toBe("2.4");
  });

  it("resolves a legacy leaf deep-link to its containing group", () => {
    expect(findGroupForArticleNumber(groups, "1.e.1°")?.number).toBe("1");
    expect(findGroupForArticleNumber(groups, "2.4.1.a.1°")?.number).toBe("2.4");
  });

  it("returns undefined for an unknown number", () => {
    expect(findGroupForArticleNumber(groups, "99")).toBeUndefined();
    expect(findGroupForArticleNumber(groups, null)).toBeUndefined();
  });
});
