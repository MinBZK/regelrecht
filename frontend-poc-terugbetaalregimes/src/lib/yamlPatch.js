/**
 * Pure helpers om een definitie-waarde in een regelrecht-YAML-tekst te
 * lezen of te vervangen, zonder de lawStore te raken. Gebruikt door het
 * parameterpaneel en door de MCP-tools van de beleidsassistent.
 */
import yaml from 'js-yaml';

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

/** De wettekst (`text:`) van één artikel, of undefined. */
export function readArticleText(yamlText, article) {
  const doc = yaml.load(yamlText);
  const art = (doc?.articles ?? []).find((a) => String(a.number) === String(article));
  return art?.text;
}

/**
 * Vervang de wettekst (`text:`) van één artikel, met behoud van de
 * blokstijl die er stond (`>-` of `|`).
 *
 * Waarom een tekstpatch en geen yaml.dump: dumpen herschrijft het hele
 * document en gooit commentaar en inspringing overhoop. De diff die de
 * gebruiker te zien krijgt moet de wetswijziging tonen, niet duizend regels
 * herschikking.
 *
 * Waarom dit bestaat: een artikel draagt zijn wettekst naast de
 * machine-leesbare regels. Werd alleen de waarde gepatcht, dan bleef de proza
 * de oude regel vertellen, en dat is in een demo over wetgeving precies de
 * verkeerde indruk: de wet is de tekst.
 */
export function patchArticleText(yamlText, article, nieuweTekst) {
  const lines = yamlText.split('\n');
  const artRe = /^(\s*)-\s*number:\s*(['"]?)(.+?)\2\s*$/;
  let start = -1;
  let itemIndent = 0;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(artRe);
    if (m && m[3] === String(article)) { start = i; itemIndent = m[1].length; break; }
  }
  if (start < 0) throw new Error(`artikel ${article} niet gevonden`);
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if (artRe.test(lines[i])) { end = i; break; }
  }

  // `text:` van dit artikel: twee spaties dieper dan het streepje van het item.
  const keyIndent = itemIndent + 2;
  let textRegel = -1;
  for (let i = start + 1; i < end; i++) {
    const m = lines[i].match(/^(\s*)text:(.*)$/);
    if (m && m[1].length === keyIndent) { textRegel = i; break; }
  }
  if (textRegel < 0) throw new Error(`artikel ${article} heeft geen wettekst om te wijzigen`);

  // De stijl aanhouden die er stond; zonder blokindicator wordt het `>-`,
  // want dat is wat het corpus overwegend gebruikt voor lopende tekst.
  const kop = lines[textRegel].match(/^(\s*)text:\s*(\S*)/);
  const stijl = /^[|>]/.test(kop[2] ?? '') ? kop[2] : '>-';

  // Alles wat bij deze waarde hoort: de ingesprongen regels eronder.
  let waardeEind = textRegel + 1;
  while (waardeEind < end) {
    const regel = lines[waardeEind];
    if (regel.trim() === '') { waardeEind++; continue; }
    if (regel.match(/^(\s*)/)[1].length <= keyIndent) break;
    waardeEind++;
  }

  const inhoudIndent = ' '.repeat(keyIndent + 2);
  const nieuweRegels = String(nieuweTekst)
    .replace(/\s+$/, '')
    .split('\n')
    .map((r) => (r.trim() ? inhoudIndent + r.trim() : ''));

  lines.splice(textRegel, waardeEind - textRegel, `${' '.repeat(keyIndent)}text: ${stijl}`, ...nieuweRegels);
  const nieuw = lines.join('\n');
  // Bewijs dat het nog geldige YAML is en dat de tekst echt is aangekomen;
  // een kapotte wet valt anders pas in de engine op.
  const doc = yaml.load(nieuw);
  const art = (doc?.articles ?? []).find((a) => String(a.number) === String(article));
  if (typeof art?.text !== 'string') throw new Error('wettekst kon niet worden weggeschreven');
  return nieuw;
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
