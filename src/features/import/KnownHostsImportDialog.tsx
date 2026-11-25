import { LoaderCircle } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { importKnownHostsApply, toAppError } from '@/lib/ipc';
import type { KnownHostsImportPreview } from '@/lib/types';

interface KnownHostsImportDialogProps {
  preview: KnownHostsImportPreview;
  onImported: () => void;
  onClose: () => void;
}

/**
 * Aperçu de l'import de `known_hosts` (IMP-02) : clés ajoutées, doublons et lignes refusées
 * (dont les conflits : une autre clé pour un hôte déjà connu n'est jamais ajoutée).
 */
export function KnownHostsImportDialog({
  preview,
  onImported,
  onClose,
}: KnownHostsImportDialogProps) {
  const { t } = useTranslation();
  const id = useId();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function apply() {
    setBusy(true);
    setError(null);
    try {
      await importKnownHostsApply(preview.previewId);
      onImported();
    } catch (failure) {
      setError(toAppError(failure).code);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal labelledBy={`${id}-title`} onClose={onClose} scope="window" className="max-w-2xl">
      <div className="flex max-h-[80vh] flex-col gap-4">
        <h2 id={`${id}-title`} className="text-lg font-semibold">
          {t('import.knownHosts.title')}
        </h2>
        <p className="text-sm text-muted-foreground">
          {t('import.knownHosts.summary', {
            added: preview.addedCount,
            duplicates: preview.duplicates,
            skipped: preview.skippedCount,
          })}
        </p>
        <p className="text-sm">{t('import.knownHosts.warning')}</p>
        {error && <ErrorMessage code={error} />}
        <div className="min-h-0 flex-1 overflow-auto">
          {preview.added.length > 0 && (
            <ul aria-label={t('import.knownHosts.added')} className="flex flex-col gap-0.5">
              {preview.added.map((line, index) => (
                <li key={`${String(index)}-${line.hosts}`} className="font-mono text-xs">
                  {line.hosts} <span className="text-muted-foreground">{line.algorithm}</span>
                </li>
              ))}
            </ul>
          )}
          {preview.skipped.length > 0 && (
            <section aria-labelledby={`${id}-skipped`} className="mt-4 flex flex-col gap-1">
              <h3 id={`${id}-skipped`} className="text-sm font-medium">
                {t('import.knownHosts.skippedTitle', { count: preview.skippedCount })}
              </h3>
              <ul className="flex flex-col gap-0.5 text-xs">
                {preview.skipped.map((line) => (
                  <li key={line.line} className="flex gap-2">
                    <span className="text-muted-foreground">
                      {t('import.knownHosts.line', { line: line.line })}
                    </span>
                    <span className="font-mono">{line.hosts}</span>
                    <span>
                      {t(`import.knownHosts.skip.${line.reason}`, { defaultValue: line.reason })}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            disabled={busy || preview.addedCount === 0}
            onClick={() => void apply()}
          >
            {busy && <LoaderCircle aria-hidden="true" className="animate-spin" />}
            {t('import.knownHosts.submit', { count: preview.addedCount })}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
