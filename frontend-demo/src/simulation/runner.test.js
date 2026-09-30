/**
 * Welke wetten de simulatie draait.
 *
 * Dezelfde afleiding als het portaal (RFC-038), want een simulatie over een
 * synthetische bevolking hoort precies te doen wat het portaal voor één
 * persoon doet. Dat liep eerder uit elkaar: de simulatie las nog het oude
 * `discoverable`, dat uit het corpus verdwenen is, en draaide daardoor stil
 * op nul wetten.
 */
import { describe, expect, it } from 'vitest';
import { simulationLaws } from './runner.js';

const law = (id, legalCharacter, params = ['bsn'], outputs = ['bedrag']) => ({
  id,
  name: id,
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            ...(legalCharacter ? { produces: { legal_character: legalCharacter } } : {}),
            parameters: params.map((name) => ({ name })),
            output: outputs.map((name) => ({ name })),
          },
        },
      },
    ],
  },
});

const corpusOf = (...laws) => ({ latestById: new Map(laws.map((l) => [l.id, l])) });

describe('simulationLaws', () => {
  it('draait de burgerwetten die een beschikking opleveren', () => {
    const corpus = corpusOf(
      law('zorgtoeslag', 'BESCHIKKING'),
      law('brp', 'INFORMATIEF'),
      law('alcoholwet', 'BESCHIKKING', ['kvk_nummer']),
    );
    const { runnable } = simulationLaws(corpus, 'burgers');
    expect(runnable.map((l) => l.id)).toEqual(['zorgtoeslag']);
  });

  it('draait de ondernemerswetten voor ondernemers', () => {
    const corpus = corpusOf(
      law('zorgtoeslag', 'BESCHIKKING'),
      law('alcoholwet', 'BESCHIKKING', ['kvk_nummer']),
    );
    const { runnable } = simulationLaws(corpus, 'ondernemers');
    expect(runnable.map((l) => l.id)).toEqual(['alcoholwet']);
  });

  it('neemt een rechtspositie mee', () => {
    // Kiesgerechtigdheid: geen besluit, wel iets wat de burger aangaat.
    const corpus = corpusOf(law('kieswet', 'RECHTSPOSITIE'));
    expect(simulationLaws(corpus, 'burgers').runnable.map((l) => l.id)).toEqual(['kieswet']);
  });

  it('laat delegatieleveranciers buiten de simulatie', () => {
    const corpus = corpusOf(
      law('gezag', 'RECHTSPOSITIE', ['bsn'], ['heeft_delegaties', 'subject_ids', 'delegation_types']),
    );
    expect(simulationLaws(corpus, 'burgers').runnable).toEqual([]);
  });

  it('volgt de zichtbaarheidskeuze van de demo', () => {
    const corpus = corpusOf(law('zorgtoeslag', 'BESCHIKKING'), law('bibob', 'BESCHIKKING'));
    const { runnable } = simulationLaws(corpus, 'burgers', (l) => l.id !== 'bibob');
    expect(runnable.map((l) => l.id)).toEqual(['zorgtoeslag']);
  });

  it('zet een wet die de bevolking niet kan voeden apart', () => {
    const corpus = corpusOf(law('vergunning', 'BESCHIKKING', ['bsn', 'lievelingskleur']));
    const { runnable, skipped } = simulationLaws(corpus, 'burgers');
    expect(runnable).toEqual([]);
    expect(skipped).toEqual([expect.objectContaining({ missing: ['lievelingskleur'] })]);
  });
});
