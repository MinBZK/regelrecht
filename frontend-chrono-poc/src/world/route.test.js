import { describe, expect, it } from 'vitest';
import {
  ACTOR_PAGE,
  actorFromHash,
  actorHref,
  currentActor,
  currentPage,
  pageFromHash,
  pages,
  WORLD_PAGE,
} from './route.js';
import { portaalFixture } from '../testing/portaalFixture.js';

describe('de pagina\'s', () => {
  it('zijn er niet zonder portaal: dan is de wereld de enige pagina', () => {
    expect(pages(null)).toStrictEqual([]);
    expect(currentPage('#/portaal', null)).toBe(WORLD_PAGE);
    expect(currentPage('', null)).toBe(WORLD_PAGE);
  });

  it('zijn er drie met een portaal, met het label uit het wereldbestand', () => {
    const list = pages(portaalFixture);
    expect(list.map((page) => page.key)).toStrictEqual(['portaal', 'inzicht', 'wereld']);
    expect(list.map((page) => page.href)).toStrictEqual(['#/portaal', '#/inzicht', '#/wereld']);
    expect(list[0].text).toBe('Aanvraagportaal');
    expect(list[2].text).toBe('Achter de schermen');
  });

  it('lezen hun adres uit de hash, en vallen bij een onbekend adres terug op het portaal', () => {
    expect(pageFromHash('#/inzicht')).toBe('inzicht');
    expect(pageFromHash('#/inzicht?x=1')).toBe('inzicht');
    expect(pageFromHash('')).toBe('');
    expect(currentPage('#/inzicht', portaalFixture)).toBe('inzicht');
    expect(currentPage('#/wereld', portaalFixture)).toBe('wereld');
    expect(currentPage('#/bestaat-niet', portaalFixture)).toBe('portaal');
    expect(currentPage('', portaalFixture)).toBe('portaal');
  });
});

describe('de pagina per actor', () => {
  const actors = ['burger', 'toeslagen'];

  it('komt erbij zodra er actoren zijn, ook zonder portaal', () => {
    expect(pages(null, actors).map((page) => page.key)).toStrictEqual([ACTOR_PAGE, WORLD_PAGE]);
    expect(pages(portaalFixture, actors).map((page) => page.key)).toStrictEqual([
      'portaal',
      'inzicht',
      ACTOR_PAGE,
      WORLD_PAGE,
    ]);
    expect(pages(null, actors)[0].href).toBe('#/actor/burger');
  });

  it('leest de actor uit het adres, en valt terug op de eerste', () => {
    expect(actorFromHash('#/actor/toeslagen')).toBe('toeslagen');
    expect(actorFromHash('#/actor/cel%20met%20spatie')).toBe('cel met spatie');
    expect(actorFromHash('#/actor')).toBe('');
    expect(actorHref('cel met spatie')).toBe('#/actor/cel%20met%20spatie');
    expect(currentPage('#/actor/toeslagen', null, actors)).toBe(ACTOR_PAGE);
    expect(currentActor('#/actor/toeslagen', actors)).toBe('toeslagen');
    expect(currentActor('#/actor/bestaat-niet', actors)).toBe('burger');
    expect(currentPage('#/actor/burger', null, [])).toBe(WORLD_PAGE);
    expect(currentPage('#/actor/burger', portaalFixture, [])).toBe('portaal');
  });
});
