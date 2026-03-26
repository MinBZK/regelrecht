/**
 * ExecutionContext — holds state for a single scenario execution.
 */
export class ExecutionContext {
  constructor() {
    /** @type {string|null} */
    this.calculationDate = null;
    /** @type {Record<string, any>} */
    this.parameters = {};
    /** @type {any} */
    this.result = null;
    /** @type {Error|null} */
    this.error = null;
    /** @type {boolean} */
    this.executed = false;
  }
}
