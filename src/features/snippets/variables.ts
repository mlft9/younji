// Variables `{{nom}}` des snippets (SNP-02). Mêmes règles que `core-vault` (model.rs) : le cœur
// valide le contenu à l'enregistrement, l'interface revalide avant chaque envoi.

/** Taille maximale du contenu d'un snippet, en octets (comme le cœur). */
export const MAX_SNIPPET_BYTES = 16 * 1024;
/** Longueur maximale d'une valeur de variable. */
export const MAX_VALUE_LENGTH = 1024;
/** Longueur maximale du texte final, variables remplacées. */
export const MAX_RENDERED_LENGTH = 64 * 1024;
/** Longueur maximale d'un nom de variable. */
const MAX_VARIABLE_NAME = 32;

/** Caractères de contrôle refusés dans un contenu (tout sauf `\t` et `\n`). */
// eslint-disable-next-line no-control-regex -- recherche volontaire des caractères de contrôle
const CONTENT_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;
/**
 * Caractères invisibles de mise en forme (largeur nulle, contrôles bidirectionnels, BOM),
 * refusés comme dans le cœur : l'aperçu montrerait un autre texte que celui qui part.
 */
const INVISIBLE_FORMAT = /[\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\ufeff]/;
/** Caractères de contrôle refusés dans une valeur (tous, retour à la ligne compris). */
// eslint-disable-next-line no-control-regex -- recherche volontaire des caractères de contrôle
const VALUE_CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

/** Vrai pour un nom de variable admis : `[A-Za-z_][A-Za-z0-9_-]{0,31}`. */
export function isVariableName(name: string): boolean {
  return name.length <= MAX_VARIABLE_NAME && /^[A-Za-z_][A-Za-z0-9_-]*$/.test(name);
}

interface Token {
  kind: 'text' | 'variable';
  value: string;
}

/**
 * Découpe un contenu en texte et variables. Un motif `{{…}}` dont le nom n'est pas admis reste
 * du texte (même algorithme que `snippet_variables` côté Rust).
 */
function tokenize(content: string): Token[] {
  const tokens: Token[] = [];
  let text = '';
  let rest = content;
  for (;;) {
    const start = rest.indexOf('{{');
    if (start === -1) break;
    const after = rest.slice(start + 2);
    const end = after.indexOf('}}');
    if (end === -1) break;
    const name = after.slice(0, end);
    if (isVariableName(name)) {
      text += rest.slice(0, start);
      if (text !== '') tokens.push({ kind: 'text', value: text });
      text = '';
      tokens.push({ kind: 'variable', value: name });
      rest = after.slice(end + 2);
    } else {
      // Pas une variable : on reprend juste après la première accolade.
      text += rest.slice(0, start + 1);
      rest = rest.slice(start + 1);
    }
  }
  text += rest;
  if (text !== '') tokens.push({ kind: 'text', value: text });
  return tokens;
}

/** Variables d'un contenu, dans l'ordre de première apparition, sans doublon. */
export function snippetVariables(content: string): string[] {
  const names: string[] = [];
  for (const token of tokenize(content)) {
    if (token.kind === 'variable' && !names.includes(token.value)) names.push(token.value);
  }
  return names;
}

/** Contenu admis : non vide, 16 Kio au plus, sans caractère de contrôle hors `\t` et `\n`. */
export function isSafeContent(content: string): boolean {
  return (
    content.trim() !== '' &&
    new TextEncoder().encode(content).length <= MAX_SNIPPET_BYTES &&
    !CONTENT_CONTROL.test(content) &&
    !INVISIBLE_FORMAT.test(content)
  );
}

/**
 * Valeur de variable admise : non vide, sur une seule ligne, sans aucun caractère de contrôle
 * (un retour à la ligne lancerait la commande avant la fin, un `ESC` injecterait une séquence).
 */
export function isSafeValue(value: string): boolean {
  return (
    value !== '' &&
    value.length <= MAX_VALUE_LENGTH &&
    !VALUE_CONTROL.test(value) &&
    !INVISIBLE_FORMAT.test(value)
  );
}

/**
 * Texte final : chaque `{{nom}}` remplacé par sa valeur, telle quelle (aucun échappement shell
 * automatique : l'aperçu montre exactement ce qui part). `null` si une valeur manque ou est
 * refusée, ou si le résultat est vide ou trop long.
 */
export function renderSnippet(content: string, values: Record<string, string>): string | null {
  const normalized = content.replace(/\r\n/g, '\n');
  let out = '';
  for (const token of tokenize(normalized)) {
    if (token.kind === 'text') {
      out += token.value;
    } else {
      const value = values[token.value];
      if (value === undefined || !isSafeValue(value)) return null;
      out += value;
    }
  }
  const safe =
    out.trim() !== '' &&
    out.length <= MAX_RENDERED_LENGTH &&
    !CONTENT_CONTROL.test(out) &&
    !INVISIBLE_FORMAT.test(out);
  return safe ? out : null;
}

/** Exemple de variable affiché dans l'aide (inséré comme valeur, donc jamais interpolé). */
export const VARIABLE_EXAMPLE = '{{service}}';
