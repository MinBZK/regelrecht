/**
 * useDataSourceSchema — extract data source schemas from loaded law YAMLs.
 *
 * Walks all loaded laws and finds inputs with `source: {}` (empty source),
 * which are leaf-level fields that the user must provide via data source tables.
 * Groups them by law and derives the key field from the article's parameters.
 */
import { ref, computed } from 'vue';
import yaml from 'js-yaml';

/**
 * @typedef {object} DataSourceField
 * @property {string} name - Field name (e.g., "geboortedatum")
 * @property {string} type - Field type (e.g., "string", "number", "boolean", "amount")
 * @property {object} [type_spec] - Type specification (e.g., { unit: "eurocent" })
 */

/**
 * @typedef {object} DataSourceGroup
 * @property {string} lawId - Law ID (e.g., "wet_basisregistratie_personen")
 * @property {string} lawName - Human-readable law name
 * @property {string} articleNumber - Article number containing the inputs
 * @property {DataSourceField[]} fields - Fields the user needs to provide
 * @property {string} keyField - Key field for record lookup (usually "bsn")
 */

/**
 * Check if a source object is empty (i.e., `source: {}`).
 * An empty source means the input should come from the DataSourceRegistry.
 */
function isEmptySource(source) {
  if (!source) return false;
  if (typeof source !== 'object') return false;
  // source: {} — has no regulation and no output
  return !source.regulation && !source.output;
}

/**
 * Extract data source groups from a parsed law object.
 *
 * @param {object} law - Parsed law YAML object
 * @returns {DataSourceGroup[]}
 */
export function extractDataSourceGroups(law) {
  const groups = [];
  const lawId = law.$id || '';
  const lawName = resolveLawName(law);

  for (const article of law.articles || []) {
    const execution = article.machine_readable?.execution;
    if (!execution) continue;

    const inputs = execution.input || [];
    const emptySourceInputs = inputs.filter((input) => isEmptySource(input.source));

    if (emptySourceInputs.length === 0) continue;

    // Derive key field from article parameters
    const params = execution.parameters || [];
    const keyField = params.length > 0 ? params[0].name : 'bsn';

    const fields = emptySourceInputs.map((input) => ({
      name: input.name,
      type: input.type || 'string',
      type_spec: input.type_spec || null,
    }));

    groups.push({
      lawId,
      lawName,
      articleNumber: String(article.number),
      fields,
      keyField,
    });
  }

  return groups;
}

/**
 * Resolve the human-readable name of a law.
 * Handles the `#output_name` convention.
 */
function resolveLawName(law) {
  const name = law.name;
  if (!name) return law.$id || 'Onbekende wet';

  if (typeof name === 'string' && name.startsWith('#')) {
    const outputName = name.slice(1);
    for (const article of law.articles || []) {
      const actions = article.machine_readable?.execution?.actions;
      if (!actions) continue;
      for (const action of actions) {
        if (action.output === outputName && typeof action.value === 'string') {
          return action.value;
        }
      }
    }
  }

  return name;
}

/**
 * Extract output definitions from the main law for the scenario form.
 *
 * @param {object} law - Parsed law YAML object
 * @returns {Array<{name: string, type: string, articleNumber: string, type_spec: object|null}>}
 */
export function extractOutputs(law) {
  const outputs = [];

  for (const article of law.articles || []) {
    const execution = article.machine_readable?.execution;
    if (!execution?.output) continue;

    // Only include articles that produce decisions (BESCHIKKING, TOEKENNING, etc.)
    const produces = execution.produces;
    if (!produces) continue;

    for (const output of execution.output) {
      outputs.push({
        name: output.name,
        type: output.type || 'string',
        articleNumber: String(article.number),
        type_spec: output.type_spec || null,
      });
    }
  }

  return outputs;
}

/**
 * Extract parameters from the main law's execution articles.
 *
 * @param {object} law - Parsed law YAML object
 * @returns {Array<{name: string, type: string, required: boolean}>}
 */
export function extractParameters(law) {
  const seen = new Set();
  const params = [];

  for (const article of law.articles || []) {
    const execution = article.machine_readable?.execution;
    if (!execution?.parameters) continue;

    for (const param of execution.parameters) {
      if (!seen.has(param.name)) {
        seen.add(param.name);
        params.push({
          name: param.name,
          type: param.type || 'string',
          required: param.required !== false,
        });
      }
    }
  }

  return params;
}

/**
 * Composable for deriving data source schemas from loaded laws.
 */
export function useDataSourceSchema() {
  const dataSourceGroups = ref([]);
  const outputs = ref([]);
  const parameters = ref([]);

  /**
   * Build schema from the main law and all loaded dependency law YAMLs.
   *
   * @param {string} mainLawYaml - Raw YAML text of the main law
   * @param {string[]} depLawIds - IDs of loaded dependency laws
   * @param {(lawId: string) => Promise<string>} fetchLawYaml - Fetch law YAML by ID
   */
  async function buildSchema(mainLawYaml, depLawIds, fetchLawYaml) {
    const allGroups = [];

    // Extract from main law
    const mainLaw = yaml.load(mainLawYaml);
    allGroups.push(...extractDataSourceGroups(mainLaw));
    outputs.value = extractOutputs(mainLaw);
    parameters.value = extractParameters(mainLaw);

    // Extract from each dependency law
    for (const lawId of depLawIds) {
      try {
        const yamlText = await fetchLawYaml(lawId);
        const depLaw = yaml.load(yamlText);
        allGroups.push(...extractDataSourceGroups(depLaw));
      } catch {
        // Skip laws that can't be parsed
      }
    }

    // Deduplicate groups by lawId + articleNumber
    const seen = new Set();
    const unique = [];
    for (const group of allGroups) {
      const key = `${group.lawId}:${group.articleNumber}`;
      if (!seen.has(key)) {
        seen.add(key);
        unique.push(group);
      }
    }

    dataSourceGroups.value = unique;
  }

  const totalFields = computed(() =>
    dataSourceGroups.value.reduce((sum, g) => sum + g.fields.length, 0),
  );

  return { dataSourceGroups, outputs, parameters, totalFields, buildSchema };
}
