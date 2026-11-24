import { type SyntheticEvent, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { snippetSave, toAppError } from '@/lib/ipc';
import type { SnippetEntry } from '@/lib/types';

import { isSafeContent, snippetVariables, VARIABLE_EXAMPLE } from './variables';

interface SnippetFormProps {
  snippet: SnippetEntry | null;
  onSaved: () => void;
  onClose: () => void;
}

/** Création ou modification d'un snippet (SNP-01). Le cœur revalide tout à l'enregistrement. */
export function SnippetForm({ snippet, onSaved, onClose }: SnippetFormProps) {
  const { t } = useTranslation();
  const id = useId();
  const [name, setName] = useState(snippet?.name ?? '');
  const [content, setContent] = useState(snippet?.content ?? '');
  const [description, setDescription] = useState(snippet?.description ?? '');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const variables = snippetVariables(content);

  async function submit(event: SyntheticEvent) {
    event.preventDefault();
    if (name.trim() === '') {
      setError('vault.invalid_name');
      return;
    }
    if (!isSafeContent(content)) {
      setError('vault.invalid_snippet');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await snippetSave({ id: snippet?.id ?? null, name, content, description });
      onSaved();
      onClose();
    } catch (failure) {
      setError(toAppError(failure).code);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal labelledBy={`${id}-title`} onClose={onClose} scope="window" className="max-w-2xl">
      <form onSubmit={(event) => void submit(event)} noValidate className="flex flex-col gap-4">
        <h2 id={`${id}-title`} className="text-lg font-semibold">
          {snippet ? t('snippets.editTitle') : t('snippets.newTitle')}
        </h2>
        {error && <ErrorMessage code={error} />}
        <label className="flex flex-col gap-1 text-sm">
          {t('snippets.name')}
          <Input
            value={name}
            maxLength={128}
            data-autofocus
            onChange={(e) => {
              setName(e.target.value);
            }}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm">
          {t('snippets.content')}
          <textarea
            value={content}
            rows={6}
            spellCheck={false}
            className="rounded-md border bg-transparent px-3 py-2 font-mono text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
            onChange={(e) => {
              setContent(e.target.value);
            }}
          />
        </label>
        <p className="text-xs text-muted-foreground">
          {t('snippets.contentHint', { example: VARIABLE_EXAMPLE })}
        </p>
        {variables.length > 0 && (
          <p className="text-xs">{t('snippets.variablesFound', { list: variables.join(', ') })}</p>
        )}
        <label className="flex flex-col gap-1 text-sm">
          {t('snippets.description')}
          <Input
            value={description}
            maxLength={512}
            onChange={(e) => {
              setDescription(e.target.value);
            }}
          />
        </label>
        <p className="text-xs text-amber-700 dark:text-amber-400">{t('snippets.noSecrets')}</p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={busy}>
            {t('common.save')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
