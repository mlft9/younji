import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { HOME_TAB_ID, useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { useUi } from '@/stores/ui';
import { useVault } from '@/stores/vault';

import { filterSnippets } from './filter';

/** Choix rapide d'un snippet à envoyer (Ctrl+Maj+S ou barre d'outils d'un panneau). */
export function SnippetPicker({ targets }: { targets: string[] }) {
  const { t } = useTranslation();
  const id = useId();
  const snippets = useVault((s) => s.snapshot.snippets);
  const close = useSnippetRun((s) => s.close);
  const openRun = useSnippetRun((s) => s.openRun);
  const [query, setQuery] = useState('');
  const visible = filterSnippets(snippets, query);

  return (
    <Modal labelledBy={`${id}-title`} onClose={close} scope="window">
      <h2 id={`${id}-title`} className="text-lg font-semibold">
        {t('snippets.pickTitle')}
      </h2>
      {snippets.length === 0 ? (
        <>
          <p className="text-sm text-muted-foreground">{t('snippets.empty')}</p>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={close}>
              {t('common.close')}
            </Button>
            <Button
              data-autofocus
              onClick={() => {
                close();
                useUi.getState().setActiveSection('snippets');
                useSessions.getState().activate(HOME_TAB_ID);
              }}
            >
              {t('snippets.manage')}
            </Button>
          </div>
        </>
      ) : (
        <>
          <Input
            type="search"
            value={query}
            data-autofocus
            aria-label={t('snippets.search')}
            placeholder={t('snippets.search')}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            onKeyDown={(e) => {
              const first = visible[0];
              if (e.key === 'Enter' && first) {
                e.preventDefault();
                openRun(first.id, targets);
              }
            }}
          />
          <ul className="flex max-h-80 flex-col gap-1 overflow-auto">
            {visible.map((snippet) => (
              <li key={snippet.id}>
                <button
                  type="button"
                  className="flex w-full flex-col items-start rounded-md px-3 py-2 text-left outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  onClick={() => {
                    openRun(snippet.id, targets);
                  }}
                >
                  <span className="font-medium">{snippet.name}</span>
                  <span className="w-full truncate font-mono text-xs text-muted-foreground">
                    {snippet.content.split('\n')[0]}
                  </span>
                </button>
              </li>
            ))}
            {visible.length === 0 && (
              <li className="px-3 py-2 text-sm text-muted-foreground">{t('snippets.noMatch')}</li>
            )}
          </ul>
        </>
      )}
    </Modal>
  );
}
