import { LoaderCircle, TriangleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { importSshConfigApply, toAppError } from '@/lib/ipc';
import type { ConfigImportPreview } from '@/lib/types';

interface SshConfigImportDialogProps {
  preview: ConfigImportPreview;
  onImported: (count: number) => void;
  onClose: () => void;
}

/**
 * Aperçu de l'import de `~/.ssh/config` (IMP-01) : hôtes à cocher (ceux déjà dans le carnet
 * décochés), avertissements par hôte et entrées ignorées avec leur raison. L'aperçu reste dans
 * le cœur ; seuls les numéros des hôtes cochés repartent.
 */
export function SshConfigImportDialog({
  preview,
  onImported,
  onClose,
}: SshConfigImportDialogProps) {
  const { t } = useTranslation();
  const id = useId();
  const [selected, setSelected] = useState<number[]>(
    preview.hosts.filter((h) => !h.alreadyInVault).map((h) => h.index),
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function toggle(index: number) {
    setSelected((current) =>
      current.includes(index) ? current.filter((i) => i !== index) : [...current, index],
    );
  }

  async function apply() {
    setBusy(true);
    setError(null);
    try {
      onImported(await importSshConfigApply(preview.previewId, selected));
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
          {t('import.config.title')}
        </h2>
        <p className="text-sm text-muted-foreground">{t('import.config.intro')}</p>
        {error && <ErrorMessage code={error} />}
        <div className="min-h-0 flex-1 overflow-auto">
          {preview.hosts.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('import.config.noHost')}</p>
          ) : (
            <ul aria-label={t('import.config.hosts')} className="flex flex-col gap-1">
              {preview.hosts.map((host) => (
                <li key={host.index} className="rounded-md border px-3 py-2">
                  <label className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      className="mt-1"
                      checked={selected.includes(host.index)}
                      onChange={() => {
                        toggle(host.index);
                      }}
                    />
                    <span className="flex min-w-0 flex-col gap-0.5">
                      <span className="text-sm font-medium">{host.alias}</span>
                      <span className="truncate font-mono text-xs text-muted-foreground">
                        {host.username}@{host.address}
                        {host.port === 22 ? '' : `:${String(host.port)}`}
                      </span>
                      {host.alreadyInVault && (
                        <span className="text-xs text-muted-foreground">
                          {t('import.config.alreadyInVault')}
                        </span>
                      )}
                      {host.warnings.map((code) => (
                        <span
                          key={code}
                          className="flex items-center gap-1 text-xs text-amber-700 dark:text-amber-400"
                        >
                          <TriangleAlert aria-hidden="true" className="size-3 shrink-0" />
                          {t(`import.warning.${code}`, {
                            files: host.identityFiles.join(', '),
                            defaultValue: code,
                          })}
                        </span>
                      ))}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          )}
          {preview.skipped.length > 0 && (
            <section aria-labelledby={`${id}-skipped`} className="mt-4 flex flex-col gap-1">
              <h3 id={`${id}-skipped`} className="text-sm font-medium">
                {t('import.config.skipped', { count: preview.skipped.length })}
              </h3>
              <ul className="flex flex-col gap-0.5 text-xs">
                {preview.skipped.map((item, index) => (
                  // Une même directive peut apparaître plusieurs fois : la position la distingue.
                  <li key={`${String(index)}-${item.entry}`} className="flex gap-2">
                    <span className="font-mono">{item.entry}</span>
                    <span className="text-muted-foreground">
                      {t(`import.skip.${item.reason}`, { defaultValue: item.reason })}
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
            disabled={busy || selected.length === 0}
            onClick={() => void apply()}
          >
            {busy && <LoaderCircle aria-hidden="true" className="animate-spin" />}
            {t('import.config.submit', { count: selected.length })}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
