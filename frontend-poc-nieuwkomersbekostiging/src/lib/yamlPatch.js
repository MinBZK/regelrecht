/**
 * Pure helpers om een definitie-waarde in een regelrecht-YAML-tekst te
 * lezen of te vervangen, zonder de lawStore te raken. Gebruikt door de
 * budgetneutraal-oplosser, die kandidaat-YAML's naar de worker stuurt.
 */
import * as yaml from 'js-yaml';

/** Lees definitions[name].value uit artikel `article` van een YAML-tekst. */
export function readDefinitionValue(yamlText, article, name) {
  const doc = yaml.load(yamlText);
  const art = (doc?.articles ?? []).find((a) => String(a.number) === String(article));
  return art?.machine_readable?.definitions?.[name]?.value;
}

/** Nieuwe YAML-tekst met definitions[name].value = value in artikel `article`. */
export function patchDefinitionValue(yamlText, article, name, value) {
  const doc = yaml.load(yamlText);
  const art = (doc?.articles ?? []).find((a) => String(a.number) === String(article));
  const def = art?.machine_readable?.definitions?.[name];
  if (!def) throw new Error(`definitie ${name} in artikel ${article} niet gevonden`);
  // Eerst als tekstpatch: één regel verandert, commentaar en opmaak blijven,
  // en de diff toont precies de wetswijziging. Lukt dat niet, dan herserialiseren.
  const patched = patchDefinitionText(yamlText, article, name, value);
  if (patched !== null) return patched;
  def.value = value;
  return yaml.dump(doc, { lineWidth: -1 });
}

/**
 * Vervang alleen de `value:`-regel van één definitie in de YAML-tekst.
 * Geeft null als het artikel, de definitie of de value-regel niet eenduidig
 * te vinden is.
 */
export function patchDefinitionText(yamlText, article, name, value) {
  const lines = yamlText.split('\n');
  const artRe = /^\s*-\s*number:\s*(['"]?)(.+?)\1\s*$/;
  let start = -1;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(artRe);
    if (m && m[2] === String(article)) { start = i; break; }
  }
  if (start < 0) return null;
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if (artRe.test(lines[i])) { end = i; break; }
  }
  const keyRe = new RegExp('^(\\s*)' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ':\\s*$');
  for (let i = start; i < end; i++) {
    const km = lines[i].match(keyRe);
    if (!km) continue;
    const indent = km[1].length;
    for (let j = i + 1; j < end; j++) {
      const lead = lines[j].match(/^(\s*)/)[1].length;
      if (lines[j].trim() && lead <= indent) break; // volgende sleutel op hetzelfde niveau
      const vm = lines[j].match(/^(\s*value:\s*)(.*?)(\s*(#.*)?)$/);
      if (vm && lead > indent) {
        const rendered = typeof value === 'string' ? JSON.stringify(value) : String(value);
        lines[j] = vm[1] + rendered + (vm[3] ?? '');
        return lines.join('\n');
      }
    }
  }
  return null;
}

/**
 * Alle numerieke definities uit een YAML-tekst, als platte lijst
 * [{article, name, value, unit, description}] (voor keuzelijsten).
 */
export function listDefinitions(yamlText) {
  const doc = yaml.load(yamlText);
  const out = [];
  for (const art of doc?.articles ?? []) {
    const defs = art?.machine_readable?.definitions ?? {};
    for (const [name, def] of Object.entries(defs)) {
      if (!def || typeof def !== 'object' || typeof def.value !== 'number') continue;
      out.push({
        article: art.number,
        name,
        value: def.value,
        unit: def.type_spec?.unit ?? null,
        description: def.description ?? null,
      });
    }
  }
  return out;
}
