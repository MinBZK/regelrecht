import { describe, it, expect } from 'vitest';
import { buildOperationTree, describeSubtitle } from './operationTree.js';

// The canonical "eerste dag van de kalendermaand volgende op": truncate first,
// add afterwards, so the day-clamping of DATE_ADD never comes into play.
const firstOfNextMonth = {
  output: 'ingangsdatum',
  value: {
    operation: 'DATE_ADD',
    date: {
      operation: 'START_OF',
      date: '$wijzigingsdatum',
      in: 'month',
    },
    months: 1,
  },
};

describe('operationTree', () => {
  describe('buildOperationTree', () => {
    it('walks into the date operand, so a nested date operation is visible', () => {
      const tree = buildOperationTree(firstOfNextMonth);

      expect(tree.map(n => n.operation)).toEqual(['DATE_ADD', 'START_OF']);
      expect(tree[1].number).toBe('1.1');
    });

    it('keeps walking the operands it already walked', () => {
      const tree = buildOperationTree({
        output: 'in_tijdvak',
        value: {
          operation: 'AND',
          conditions: [
            { operation: 'LESS_THAN_OR_EQUAL', subject: '$begin', value: '$peildatum' },
            { operation: 'LESS_THAN_OR_EQUAL', subject: '$peildatum', value: '$einde' },
          ],
        },
      });

      expect(tree).toHaveLength(3);
    });
  });

  describe('describeSubtitle', () => {
    it('names the component DATE_PART reads', () => {
      expect(describeSubtitle({ operation: 'DATE_PART', date: '$peildatum', in: 'year' }))
        .toBe('jaar uit $peildatum');
      expect(describeSubtitle({ operation: 'DATE_PART', date: '$wijzigingsdatum', in: 'day' }))
        .toBe('dagnummer uit $wijzigingsdatum');
    });

    it('names the unit START_OF truncates to', () => {
      expect(describeSubtitle({ operation: 'START_OF', date: '$peildatum', in: 'month' }))
        .toBe('begin van de maand van $peildatum');
    });

    it('reads the operand of DATE_ADD out of its date field', () => {
      expect(describeSubtitle({ operation: 'DATE_ADD', date: '$geboortedatum', years: 18 }))
        .toBe('$geboortedatum + offset');
    });
  });
});
