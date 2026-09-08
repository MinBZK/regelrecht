// Engine-agnostic Gherkin runner for the canonical BDD grammar
// (bdd/grammar.yaml): parser, generated step patterns, and the single JS
// dispatch that gives them meaning against a WasmEngine. Shared by the editor
// (frontend/) and the demo (frontend-demo/).
export { parseFeature } from './parser.js';
export { GRAMMAR, VALUE_TYPING } from './grammar.generated.js';
export { createStepDefinitions, SUPPORTED_TIERS } from './steps.js';
export {
  dispatch,
  parseValue,
  quotedValue,
  bareValue,
  tableCellValue,
  tableToRecords,
  getOutput,
  primitiveEqual,
} from './actions.js';
export { ExecutionContext } from './context.js';
