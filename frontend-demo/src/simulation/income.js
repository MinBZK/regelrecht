/**
 * Disposable income per simulated citizen, as the POC summed it up: monthly
 * income, minus taxes, plus the benefits the laws grant, and once more after
 * housing costs. Which law outputs count, and whether a law states them per
 * month or per year, is configuration (`simulation.disposable_income` in
 * demo-config.yaml): the laws themselves do not declare a period.
 */
import { mean, median } from './stats.js';

/**
 * @typedef {object} IncomeComponent
 * @property {string} law       law id
 * @property {string} output    output name (an amount in eurocent)
 * @property {'month'|'year'} period
 * @property {'benefit'|'tax'} kind
 * @property {string} [label]   how to name it; defaults to the law name
 */

/** Monthly euro value of one output for one result, or 0 when absent. */
export function monthlyValue(result, component) {
  const law = result.laws?.[component.law];
  if (!law?.ok || law.met === false) return 0;
  const raw = law.outputs?.[component.output];
  if (typeof raw !== 'number') return 0;
  const euro = raw / 100;
  return component.period === 'year' ? euro / 12 : euro;
}

/** Monthly housing costs: the rent, or 30% of income for an owner (the POC's rule). */
export function housingCosts(subject) {
  if (subject.huurder) return subject.huur ?? 0;
  return ((subject.inkomen ?? 0) / 12) * 0.3;
}

/**
 * Per subject: income, taxes, benefits, disposable income and the same after
 * housing costs, all per month in euro.
 */
export function disposableIncomeOf(result, components) {
  const income = (result.subject.inkomen ?? 0) / 12;
  let taxes = 0;
  let benefits = 0;
  const parts = {};
  for (const c of components) {
    const v = monthlyValue(result, c);
    parts[`${c.law}#${c.output}`] = v;
    if (c.kind === 'tax') taxes += v;
    else benefits += v;
  }
  const disposable = income - taxes + benefits;
  const housing = housingCosts(result.subject);
  return { income, taxes, benefits, parts, disposable, afterHousing: disposable - housing, housing };
}

/**
 * Summary over a run: averages and medians, and the average of every component.
 * @returns {{ count, avgIncome, avgTaxes, avgBenefits, avgDisposable, medianDisposable, avgAfterHousing, avgHousing, components: Array<{component, avg, withValue}> }}
 */
export function summariseDisposableIncome(results, components) {
  const rows = results.map((r) => disposableIncomeOf(r, components));
  return {
    count: rows.length,
    avgIncome: mean(rows.map((r) => r.income)),
    avgTaxes: mean(rows.map((r) => r.taxes)),
    avgBenefits: mean(rows.map((r) => r.benefits)),
    avgDisposable: mean(rows.map((r) => r.disposable)),
    medianDisposable: median(rows.map((r) => r.disposable)),
    avgAfterHousing: mean(rows.map((r) => r.afterHousing)),
    avgHousing: mean(rows.map((r) => r.housing)),
    components: components.map((component) => {
      const key = `${component.law}#${component.output}`;
      const values = rows.map((r) => r.parts[key]);
      return { component, avg: mean(values), withValue: values.filter((v) => v !== 0).length };
    }),
  };
}

/** The summary per group of a demographic dimension. */
export function disposableIncomeBreakdown(results, components, groupOf, order = null) {
  const groups = new Map();
  for (const r of results) {
    const g = String(groupOf(r.subject));
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g).push(r);
  }
  const keys = order ? order.filter((k) => groups.has(k)) : [...groups.keys()].sort();
  return keys.map((key) => ({ group: key, ...summariseDisposableIncome(groups.get(key), components) }));
}
