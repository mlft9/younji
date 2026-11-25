import { LoaderCircle } from 'lucide-react';
import { type SyntheticEvent, useCallback, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { PasswordStrength } from '@/features/vault/PasswordStrength';
import {
  characterCount,
  MIN_MASTER_LENGTH,
  meetsPolicy,
  type StrengthScore,
} from '@/features/vault/strength';
import { backupExport, backupImport, toAppError } from '@/lib/ipc';
import type { BackupImportResult, BackupMode } from '@/lib/types';
import { useVault } from '@/stores/vault';

/**
 * Sauvegarde chiffrée (VLT-06) : export avec une phrase de passe **distincte** du mot de passe
 * maître (même exigence de robustesse, vérifiée par le cœur), import en fusion (par défaut) ou
 * en remplacement (confirmé). Les champs sont vidés dès l'envoi.
 */
export function BackupSection() {
  const { t } = useTranslation();
  const id = useId();
  const refresh = useVault((s) => s.refresh);

  const [exportPass, setExportPass] = useState('');
  const [exportConfirm, setExportConfirm] = useState('');
  const [score, setScore] = useState<StrengthScore>(0);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exported, setExported] = useState(false);
  const [exporting, setExporting] = useState(false);
  const onScore = useCallback((value: StrengthScore) => {
    setScore(value);
  }, []);

  const [importPass, setImportPass] = useState('');
  const [mode, setMode] = useState<BackupMode>('merge');
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [result, setResult] = useState<BackupImportResult | null>(null);
  const [importing, setImporting] = useState(false);

  async function submitExport(event: SyntheticEvent) {
    event.preventDefault();
    setExported(false);
    if (characterCount(exportPass) < MIN_MASTER_LENGTH) {
      setExportError('vault.password_too_short');
      return;
    }
    if (!meetsPolicy(exportPass, score)) {
      setExportError('vault.password_too_weak');
      return;
    }
    if (exportPass !== exportConfirm) {
      setExportError('vault.password_mismatch');
      return;
    }
    const passphrase = exportPass;
    setExportPass('');
    setExportConfirm('');
    setExportError(null);
    setExporting(true);
    try {
      setExported(await backupExport(passphrase));
    } catch (failure) {
      setExportError(toAppError(failure).code);
    } finally {
      setExporting(false);
    }
  }

  async function runImport() {
    const passphrase = importPass;
    setImportPass('');
    setImportError(null);
    setResult(null);
    setImporting(true);
    try {
      const imported = await backupImport(passphrase, mode);
      if (imported) {
        setResult(imported);
        await refresh();
      }
    } catch (failure) {
      setImportError(toAppError(failure).code);
    } finally {
      setImporting(false);
    }
  }

  function submitImport(event: SyntheticEvent) {
    event.preventDefault();
    if (importPass === '') {
      setImportError('backup.passphrase_required');
      return;
    }
    if (mode === 'replace') {
      setConfirmReplace(true);
    } else {
      void runImport();
    }
  }

  const passwordInput = (
    suffix: string,
    label: string,
    value: string,
    set: (value: string) => void,
    autoComplete: string,
    describedBy?: string,
  ) => (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={`${id}-${suffix}`} className="text-sm font-medium">
        {label}
      </label>
      <Input
        id={`${id}-${suffix}`}
        type="password"
        value={value}
        onChange={(e) => {
          set(e.target.value);
        }}
        autoComplete={autoComplete}
        maxLength={1024}
        className="max-w-xs"
        aria-describedby={describedBy}
      />
    </div>
  );

  return (
    <section aria-labelledby={`${id}-heading`} className="flex flex-col gap-5">
      <h2 id={`${id}-heading`} className="text-sm font-semibold text-muted-foreground">
        {t('backup.title')}
      </h2>
      <p className="text-sm text-muted-foreground">{t('backup.intro')}</p>

      <form
        onSubmit={(event) => void submitExport(event)}
        noValidate
        aria-labelledby={`${id}-export`}
        className="flex flex-col gap-3"
      >
        <h3 id={`${id}-export`} className="text-sm font-medium">
          {t('backup.export')}
        </h3>
        {exportError && <ErrorMessage code={exportError} />}
        {exported && (
          <p role="status" className="text-sm text-emerald-700 dark:text-emerald-400">
            {t('backup.exported')}
          </p>
        )}
        {passwordInput(
          'export-pass',
          t('backup.passphrase'),
          exportPass,
          setExportPass,
          'new-password',
          `${id}-strength`,
        )}
        <div className="max-w-xs">
          <PasswordStrength id={`${id}-strength`} password={exportPass} onScore={onScore} />
        </div>
        {passwordInput(
          'export-confirm',
          t('backup.confirm'),
          exportConfirm,
          setExportConfirm,
          'new-password',
        )}
        <p className="text-xs text-muted-foreground">{t('backup.distinctHint')}</p>
        <Button type="submit" variant="outline" className="self-start" disabled={exporting}>
          {exporting && <LoaderCircle aria-hidden="true" className="animate-spin" />}
          {t('backup.exportSubmit')}
        </Button>
      </form>

      <form
        onSubmit={submitImport}
        noValidate
        aria-labelledby={`${id}-import`}
        className="flex flex-col gap-3"
      >
        <h3 id={`${id}-import`} className="text-sm font-medium">
          {t('backup.import')}
        </h3>
        {importError && <ErrorMessage code={importError} />}
        {result && (
          <p role="status" className="text-sm text-emerald-700 dark:text-emerald-400">
            {t('backup.imported', { ...result })}
          </p>
        )}
        {passwordInput('import-pass', t('backup.passphrase'), importPass, setImportPass, 'off')}
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-sm font-medium">{t('backup.mode')}</legend>
          {(['merge', 'replace'] as const).map((value) => (
            <label key={value} className="flex items-start gap-2 text-sm">
              <input
                type="radio"
                name={`${id}-mode`}
                className="mt-1"
                checked={mode === value}
                onChange={() => {
                  setMode(value);
                }}
              />
              <span>
                {t(`backup.modes.${value}`)}
                <span className="block text-xs text-muted-foreground">
                  {t(`backup.modes.${value}Hint`)}
                </span>
              </span>
            </label>
          ))}
        </fieldset>
        <Button type="submit" variant="outline" className="self-start" disabled={importing}>
          {importing && <LoaderCircle aria-hidden="true" className="animate-spin" />}
          {t('backup.importSubmit')}
        </Button>
      </form>

      {confirmReplace && (
        <ConfirmDialog
          title={t('backup.replaceTitle')}
          message={t('backup.replaceMessage')}
          confirmLabel={t('backup.replaceConfirm')}
          destructive
          onCancel={() => {
            setConfirmReplace(false);
          }}
          onConfirm={() => {
            setConfirmReplace(false);
            void runImport();
          }}
        />
      )}
    </section>
  );
}
