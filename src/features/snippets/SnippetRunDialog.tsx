import { CornerDownLeft, TextCursorInput } from 'lucide-react';
import { type SyntheticEvent, useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { SessionChecklist } from '@/components/SessionChecklist';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { existingTerminalSession } from '@/features/terminal/TerminalSession';
import type { EntryId } from '@/lib/types';
import { useSessions } from '@/stores/sessions';
import { useSnippetRun } from '@/stores/snippetRun';
import { useVault } from '@/stores/vault';

import { isSafeValue, MAX_VALUE_LENGTH, renderSnippet, snippetVariables } from './variables';

/**
 * Envoi d'un snippet (SNP-02, SNP-03) : chaque variable est demandée, le texte final est
 * affiché tel qu'il partira, et les sessions visées sont cochées une à une. « Insérer » colle
 * sans valider ; « Exécuter » ajoute Entrée. L'envoi ignore la diffusion : seules les sessions
 * cochées reçoivent le texte.
 */
export function SnippetRunDialog({ run }: { run: { snippetId: EntryId; targets: string[] } }) {
  const { t } = useTranslation();
  const id = useId();
  const snippet = useVault((s) => s.snapshot.snippets.find((x) => x.id === run.snippetId));
  const close = useSnippetRun((s) => s.close);
  const tabs = useSessions((s) => s.tabs);
  const connected = tabs.filter(
    (tab) => tab.kind === 'terminal' && tab.local === null && tab.status === 'connected',
  );
  const [values, setValues] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<string[]>(run.targets);
  const [error, setError] = useState<string | null>(null);

  // Snippet supprimé ou coffre verrouillé pendant que le dialogue est ouvert.
  useEffect(() => {
    if (!snippet) close();
  }, [snippet, close]);
  if (!snippet) return null;

  // Variables recalculées par l'interface (mêmes règles que le cœur) : c'est ce texte qui part.
  const variables = snippetVariables(snippet.content);
  const targets = selected.filter((s) => connected.some((tab) => tab.id === s));
  const preview = renderSnippet(snippet.content, values);
  const multiline = preview?.includes('\n') ?? snippet.content.includes('\n');

  function send(execute: boolean) {
    if (preview === null) {
      setError('snippets.invalid_values');
      return;
    }
    if (targets.length === 0) {
      setError('snippets.no_target');
      return;
    }
    let failed = 0;
    for (const target of targets) {
      if (!existingTerminalSession(target)?.sendText(preview, execute)) failed += 1;
    }
    if (failed > 0) {
      setError('snippets.send_failed');
      return;
    }
    close();
    const first = targets[0];
    if (first) {
      useSessions.getState().focusPane(first);
      existingTerminalSession(first)?.focus();
    }
  }

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    send(false);
  }

  return (
    <Modal labelledBy={`${id}-title`} onClose={close} scope="window" className="max-w-2xl">
      <form onSubmit={submit} noValidate className="flex flex-col gap-4">
        <h2 id={`${id}-title`} className="text-lg font-semibold">
          {t('snippets.runTitle', { name: snippet.name })}
        </h2>
        {error && <ErrorMessage code={error} />}
        {variables.length > 0 && (
          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-sm font-medium">{t('snippets.variables')}</legend>
            {variables.map((name, index) => {
              const value = values[name] ?? '';
              const invalid = value !== '' && !isSafeValue(value);
              return (
                <label key={name} className="flex flex-col gap-1 text-sm">
                  <span className="font-mono">{name}</span>
                  <Input
                    value={value}
                    maxLength={MAX_VALUE_LENGTH}
                    spellCheck={false}
                    aria-invalid={invalid}
                    data-autofocus={index === 0 ? true : undefined}
                    onChange={(e) => {
                      setValues({ ...values, [name]: e.target.value });
                    }}
                  />
                </label>
              );
            })}
          </fieldset>
        )}
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">{t('snippets.preview')}</span>
          <pre
            aria-live="polite"
            className="max-h-48 overflow-auto rounded-md border bg-muted px-3 py-2 font-mono text-xs whitespace-pre-wrap"
          >
            {preview ?? t('snippets.previewPending')}
          </pre>
          {multiline && (
            <p className="text-xs text-amber-700 dark:text-amber-400">
              {t('snippets.multilineWarning')}
            </p>
          )}
        </div>
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">{t('snippets.targets')}</span>
          <SessionChecklist
            sessions={connected}
            selected={targets}
            onChange={setSelected}
            label={t('snippets.targets')}
          />
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          <Button type="button" variant="outline" onClick={close}>
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            variant="outline"
            disabled={preview === null || targets.length === 0}
            data-autofocus={variables.length === 0 ? true : undefined}
          >
            <TextCursorInput aria-hidden="true" />
            {t('snippets.insert', { count: targets.length })}
          </Button>
          <Button
            type="button"
            disabled={preview === null || targets.length === 0}
            onClick={() => {
              send(true);
            }}
          >
            <CornerDownLeft aria-hidden="true" />
            {t('snippets.execute', { count: targets.length })}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
