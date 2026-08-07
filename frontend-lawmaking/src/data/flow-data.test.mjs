// Structurele poort op de drie flow-datasets, in dezelfde vorm als
// script/cross-law-integriteit.py: een verwijzing die nergens op slaat is een
// modelleringsfout en maakt de test rood.
//
// Deze app heeft verder geen tests. De componenten renderen wat er in de data
// staat en klagen nergens over: een verbinding naar een stage die niet bestaat
// tekent gewoon niets, en een stage op een branch die niet bestaat krijgt geen
// kleur. Beide zijn onzichtbaar tot iemand het diagram naast de bedoeling legt.

import { test } from 'node:test';
import assert from 'node:assert/strict';

const DATASETS = ['flowDataSimple.js', 'flowDataAdvanced.js', 'flowDataWoo.js'];

const loaded = await Promise.all(
  DATASETS.map(async (file) => {
    const m = await import(`./${file}`);
    return {
      file,
      branches: m.branches ?? [],
      stages: m.stages ?? [],
      connections: m.connections ?? [],
    };
  }),
);

test('elke dataset heeft branches, stages en verbindingen', () => {
  // Zonder deze assertie zegt een groene run niets over een dataset die per
  // ongeluk leeg importeert.
  for (const { file, branches, stages, connections } of loaded) {
    assert.ok(branches.length > 0, `${file} heeft geen branches`);
    assert.ok(stages.length > 0, `${file} heeft geen stages`);
    assert.ok(connections.length > 0, `${file} heeft geen verbindingen`);
  }
});

test('stage-ids zijn uniek binnen een dataset', () => {
  for (const { file, stages } of loaded) {
    const seen = new Set();
    for (const s of stages) {
      assert.ok(!seen.has(s.id), `${file}: stage-id "${s.id}" komt meer dan eens voor`);
      seen.add(s.id);
    }
  }
});

test('branch-ids zijn uniek binnen een dataset', () => {
  for (const { file, branches } of loaded) {
    const seen = new Set();
    for (const b of branches) {
      assert.ok(!seen.has(b.id), `${file}: branch-id "${b.id}" komt meer dan eens voor`);
      seen.add(b.id);
    }
  }
});

test('elke stage staat op een branch die bestaat', () => {
  for (const { file, branches, stages } of loaded) {
    const ids = new Set(branches.map((b) => b.id));
    for (const s of stages) {
      assert.ok(ids.has(s.branch), `${file}: stage "${s.id}" staat op onbekende branch "${s.branch}"`);
    }
  }
});

test('elke verbinding wijst aan beide kanten naar een bestaande stage', () => {
  for (const { file, stages, connections } of loaded) {
    const ids = new Set(stages.map((s) => s.id));
    for (const c of connections) {
      assert.ok(ids.has(c.from), `${file}: verbinding vanaf onbekende stage "${c.from}"`);
      assert.ok(ids.has(c.to), `${file}: verbinding naar onbekende stage "${c.to}"`);
    }
  }
});

test('geen enkele stage verwijst naar zichzelf', () => {
  for (const { file, connections } of loaded) {
    for (const c of connections) {
      assert.notEqual(c.from, c.to, `${file}: stage "${c.from}" verwijst naar zichzelf`);
    }
  }
});

test('een stage zonder inkomende verbinding staat op de trunk', () => {
  // FlowDiagram legt rijen vast langs de inkomende verbindingen; zonder zo'n
  // verbinding valt een stage terug op de globale volgorde. Op de trunk is dat
  // gewenst (main loopt door naast het voorstel), op elke andere kolom betekent
  // het dat de aftakking naar die stage ontbreekt.
  for (const { file, stages, connections } of loaded) {
    const reachable = new Set(connections.map((c) => c.to));
    const zwevend = stages.filter((s) => !reachable.has(s.id) && s.col !== 0);
    assert.deepEqual(
      zwevend.map((s) => s.id),
      [],
      `${file}: stage(s) buiten de trunk zonder inkomende verbinding`,
    );
  }
});

test('elke stage heeft een numerieke step en col', () => {
  for (const { file, stages } of loaded) {
    for (const s of stages) {
      assert.equal(typeof s.step, 'number', `${file}: stage "${s.id}" heeft geen numerieke step`);
      assert.equal(typeof s.col, 'number', `${file}: stage "${s.id}" heeft geen numerieke col`);
    }
  }
});
