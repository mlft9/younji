import {
  isSafeContent,
  isSafeValue,
  isVariableName,
  MAX_SNIPPET_BYTES,
  renderSnippet,
  snippetVariables,
} from './variables';

describe('variables des snippets (SNP-02)', () => {
  it('extrait les variables comme le cœur', () => {
    // Même cas que le test Rust `snippet_variables_are_extracted`.
    expect(snippetVariables('a {{service}} {{ x }} {{_n-1}} {{service}} {{9a}} {{{{b}}')).toEqual([
      'service',
      '_n-1',
      'b',
    ]);
    expect(snippetVariables('{{a}')).toEqual([]);
    expect(snippetVariables('{{}} {{é}}')).toEqual([]);
    expect(isVariableName('a'.repeat(33))).toBe(false);
  });

  it('remplace chaque occurrence par sa valeur, telle quelle', () => {
    expect(
      renderSnippet('systemctl restart {{service}} && journalctl -u {{service}} {{ x }}', {
        service: 'nginx',
      }),
    ).toBe('systemctl restart nginx && journalctl -u nginx {{ x }}');
    expect(renderSnippet('a\r\nb', {})).toBe('a\nb');
  });

  it('refuse une valeur manquante, vide, sur plusieurs lignes ou avec des contrôles', () => {
    const content = 'systemctl restart {{service}}';
    expect(renderSnippet(content, {})).toBeNull();
    expect(renderSnippet(content, { service: '' })).toBeNull();
    expect(renderSnippet(content, { service: 'nginx\nreboot' })).toBeNull();
    expect(renderSnippet(content, { service: 'x\u001b[201~' })).toBeNull();
    expect(renderSnippet(content, { service: 'x\u009b' })).toBeNull();
    expect(isSafeValue('a'.repeat(1025))).toBe(false);
    expect(isSafeValue('nginx; ls')).toBe(true);
  });

  it('valide le contenu comme le cœur', () => {
    expect(isSafeContent('ls\n\tpwd')).toBe(true);
    expect(isSafeContent('  \n')).toBe(false);
    expect(isSafeContent('echo \u001b[31m')).toBe(false);
    expect(isSafeContent('a\rb')).toBe(false);
    expect(isSafeContent('é'.repeat(MAX_SNIPPET_BYTES / 2 + 1))).toBe(false);
    expect(renderSnippet('echo \u0007', {})).toBeNull();
  });
});
