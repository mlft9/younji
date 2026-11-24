import type { SnippetEntry } from '@/lib/types';

/** Snippets triés par nom et filtrés (nom, description ou contenu, sans tenir compte de la casse). */
export function filterSnippets(snippets: SnippetEntry[], query: string): SnippetEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  return snippets
    .filter(
      (s) =>
        needle === '' ||
        s.name.toLocaleLowerCase().includes(needle) ||
        s.description.toLocaleLowerCase().includes(needle) ||
        s.content.toLocaleLowerCase().includes(needle),
    )
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }));
}
