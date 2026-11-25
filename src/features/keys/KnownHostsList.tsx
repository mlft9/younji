import { FileInput, RotateCw, ShieldOff, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { KnownHostsImportDialog } from '@/features/import/KnownHostsImportDialog';
import { importKnownHostsPreview, knownHostDelete, knownHostList, toAppError } from '@/lib/ipc';
import type { ImportSource, KnownHostEntry, KnownHostsImportPreview } from '@/lib/types';

/**
 * Clés d'hôte connues (HKY-04). Supprimer une clé est le seul moyen de débloquer un hôte dont
 * la clé a changé (HKY-02) : la suppression passe par un avertissement explicite.
 */
export function KnownHostsList() {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<KnownHostEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<KnownHostEntry | null>(null);
  /** Aperçu de l'import de known_hosts (IMP-02), gardé par le cœur. */
  const [importPreview, setImportPreview] = useState<KnownHostsImportPreview | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function previewImport(source: ImportSource) {
    setNotice(null);
    try {
      const preview = await importKnownHostsPreview(source);
      if (preview) setImportPreview(preview);
      setError(null);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
  }

  const load = useCallback(async () => {
    try {
      setEntries(await knownHostList());
      setError(null);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    knownHostList()
      .then((list) => {
        if (!cancelled) setEntries(list);
      })
      .catch((failure: unknown) => {
        if (!cancelled) setError(toAppError(failure).code);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function remove(entry: KnownHostEntry) {
    try {
      await knownHostDelete(entry);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
    await load();
  }

  return (
    <section aria-labelledby="known-hosts-title" className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <h2 id="known-hosts-title" className="text-base font-semibold">
          {t('knownHosts.title')}
        </h2>
        <Button
          size="icon"
          variant="ghost"
          className="ml-auto size-8"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          onClick={() => void load()}
        >
          <RotateCw aria-hidden="true" />
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">{t('knownHosts.intro')}</p>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" variant="outline" onClick={() => void previewImport('default')}>
          <FileInput aria-hidden="true" />
          {t('import.knownHosts.fromDefault')}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void previewImport('pick')}>
          {t('import.fromOtherFile')}
        </Button>
      </div>
      {error && <ErrorMessage code={error} />}
      {notice && (
        <p role="status" className="text-sm text-emerald-700 dark:text-emerald-400">
          {notice}
        </p>
      )}
      {entries?.length === 0 && (
        <p className="text-sm text-muted-foreground">{t('knownHosts.empty')}</p>
      )}
      {entries && entries.length > 0 && (
        <ul aria-label={t('knownHosts.title')} className="flex flex-col gap-1">
          {entries.map((entry) => (
            // Position de la ligne : unique, même si deux lignes sont identiques.
            <li key={entry.line} className="flex items-center gap-3 rounded-md border px-3 py-2">
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="flex items-center gap-2 truncate font-mono text-sm">
                  {entry.revoked && (
                    <span className="flex items-center gap-1 rounded bg-destructive/10 px-1.5 text-xs text-destructive">
                      <ShieldOff aria-hidden="true" className="size-3" />
                      {t('knownHosts.revoked')}
                    </span>
                  )}
                  {entry.hosts}
                </span>
                <span className="truncate font-mono text-xs text-muted-foreground">
                  {entry.algorithm} {entry.fingerprint || t('hostKey.unreadable')}
                </span>
              </span>
              <Button
                size="icon"
                variant="ghost"
                className="size-8"
                aria-label={t('knownHosts.delete', { host: entry.hosts })}
                title={t('common.delete')}
                onClick={() => {
                  setToDelete(entry);
                }}
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      {toDelete && (
        <ConfirmDialog
          title={t('knownHosts.deleteTitle')}
          message={t('knownHosts.deleteMessage', {
            host: toDelete.hosts,
            fingerprint: toDelete.fingerprint || t('hostKey.unreadable'),
          })}
          confirmLabel={t('knownHosts.deleteConfirm')}
          destructive
          onCancel={() => {
            setToDelete(null);
          }}
          onConfirm={() => {
            const entry = toDelete;
            setToDelete(null);
            void remove(entry);
          }}
        />
      )}
      {importPreview && (
        <KnownHostsImportDialog
          preview={importPreview}
          onClose={() => {
            setImportPreview(null);
          }}
          onImported={() => {
            setImportPreview(null);
            setNotice(t('import.doneKnownHosts'));
            void load();
          }}
        />
      )}
    </section>
  );
}
