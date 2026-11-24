import { Pencil, Plus, Search, Send, SquareTerminal, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { snippetDelete, toAppError } from '@/lib/ipc';
import type { SnippetEntry } from '@/lib/types';
import { useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { useVault } from '@/stores/vault';

import { filterSnippets } from './filter';
import { VARIABLE_EXAMPLE } from './variables';
import { SnippetForm } from './SnippetForm';

type Dialog =
  { kind: 'edit'; snippet: SnippetEntry | null } | { kind: 'delete'; snippet: SnippetEntry };

/** Bibliothèque de snippets (SNP-01) : recherche, création, modification, suppression, envoi. */
export function SnippetsView() {
  const { t } = useTranslation();
  const snippets = useVault((s) => s.snapshot.snippets);
  const refresh = useVault((s) => s.refresh);
  const [query, setQuery] = useState('');
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const visible = filterSnippets(snippets, query);

  async function remove(snippet: SnippetEntry) {
    setDialog(null);
    setError(null);
    try {
      await snippetDelete(snippet.id);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
    await refresh();
  }

  function send(snippet: SnippetEntry) {
    // Cible proposée : le dernier panneau utilisé, s'il est encore connecté.
    const { lastPaneId, tabs } = useSessions.getState();
    const last = tabs.find((tab) => tab.id === lastPaneId && tab.status === 'connected');
    useSnippetRun.getState().openRun(snippet.id, last ? [last.id] : []);
  }

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">{t('nav.snippets')}</h1>
        <Button
          onClick={() => {
            setDialog({ kind: 'edit', snippet: null });
          }}
        >
          <Plus aria-hidden="true" />
          {t('snippets.new')}
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">
        {t('snippets.intro', { example: VARIABLE_EXAMPLE })}
      </p>
      <div className="relative">
        <Search
          aria-hidden="true"
          className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          type="search"
          value={query}
          aria-label={t('snippets.search')}
          placeholder={t('snippets.search')}
          className="pl-8"
          onChange={(e) => {
            setQuery(e.target.value);
          }}
        />
      </div>
      {error && <ErrorMessage code={error} />}
      {snippets.length === 0 ? (
        <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center">
          <SquareTerminal aria-hidden="true" className="size-8 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t('snippets.empty')}</p>
        </div>
      ) : visible.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('snippets.noMatch')}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {visible.map((snippet) => (
            <li key={snippet.id} className="flex items-start gap-3 rounded-lg border p-3">
              <div className="min-w-0 flex-1">
                <p className="font-medium">{snippet.name}</p>
                {snippet.description !== '' && (
                  <p className="text-sm text-muted-foreground">{snippet.description}</p>
                )}
                <pre className="mt-1 max-h-24 overflow-auto rounded bg-muted px-2 py-1 font-mono text-xs whitespace-pre-wrap">
                  {snippet.content}
                </pre>
              </div>
              <div className="flex shrink-0 gap-1">
                <Button
                  size="icon"
                  variant="outline"
                  className="size-8"
                  aria-label={t('snippets.send', { name: snippet.name })}
                  title={t('snippets.sendShort')}
                  onClick={() => {
                    send(snippet);
                  }}
                >
                  <Send aria-hidden="true" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('snippets.edit', { name: snippet.name })}
                  title={t('common.edit')}
                  onClick={() => {
                    setDialog({ kind: 'edit', snippet });
                  }}
                >
                  <Pencil aria-hidden="true" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('snippets.delete', { name: snippet.name })}
                  title={t('common.delete')}
                  onClick={() => {
                    setDialog({ kind: 'delete', snippet });
                  }}
                >
                  <Trash2 aria-hidden="true" />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {dialog?.kind === 'edit' && (
        <SnippetForm
          snippet={dialog.snippet}
          onSaved={() => void refresh()}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'delete' && (
        <ConfirmDialog
          title={t('snippets.deleteTitle')}
          message={t('snippets.deleteMessage', { name: dialog.snippet.name })}
          confirmLabel={t('common.delete')}
          destructive
          onConfirm={() => void remove(dialog.snippet)}
          onCancel={() => {
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}
