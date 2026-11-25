import {
  Copy,
  Download,
  KeyRound,
  Pencil,
  Plus,
  Send,
  Sparkles,
  Trash2,
  Upload,
  UserRound,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '@/components/ConfirmDialog';
import { NameDialog } from '@/components/NameDialog';
import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { identityDelete, keyDelete, keyExportPublic, keyRename, toAppError } from '@/lib/ipc';
import type { IdentityEntry, KeyEntry } from '@/lib/types';
import { useVault } from '@/stores/vault';

import { IdentityForm } from './IdentityForm';
import { KeyDeployDialog } from './KeyDeployDialog';
import { KeyGenerateDialog } from './KeyGenerateDialog';
import { KeyImportDialog } from './KeyImportDialog';
import { KnownHostsList } from './KnownHostsList';

type Dialog =
  | { kind: 'import' }
  | { kind: 'generate' }
  | { kind: 'deploy'; key: KeyEntry }
  | { kind: 'renameKey'; key: KeyEntry }
  | { kind: 'deleteKey'; key: KeyEntry }
  | { kind: 'identity'; identity: IdentityEntry | null }
  | { kind: 'deleteIdentity'; identity: IdentityEntry };

/**
 * Section Clés : clés générées (KEY-01) ou importées (KEY-02), export et déploiement de la clé
 * publique (KEY-03), identités (HST-03), clés d'hôte connues (HKY-04).
 */
export function KeysView() {
  const { t } = useTranslation();
  const snapshot = useVault((s) => s.snapshot);
  const refresh = useVault((s) => s.refresh);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [exported, setExported] = useState<string | null>(null);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
    } catch (failure) {
      setError(toAppError(failure).code);
    } finally {
      await refresh();
    }
  }

  async function copyPublicKey(key: KeyEntry) {
    try {
      // Clé publique uniquement : rien de secret ne part dans le presse-papiers.
      await navigator.clipboard.writeText(key.publicKey);
      setCopied(key.id);
    } catch {
      setError('keys.copy_failed');
    }
  }

  async function exportPublicKey(key: KeyEntry) {
    setError(null);
    try {
      if (await keyExportPublic(key.id)) setExported(key.id);
    } catch (failure) {
      setError(toAppError(failure).code);
    }
  }

  const close = () => {
    setDialog(null);
  };

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8 p-6">
      <h1 className="text-xl font-semibold">{t('nav.keys')}</h1>
      {error && <ErrorMessage code={error} />}

      <section aria-labelledby="keys-title" className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <h2 id="keys-title" className="text-base font-semibold">
            {t('keys.title')}
          </h2>
          <Button
            size="sm"
            className="ml-auto"
            onClick={() => {
              setDialog({ kind: 'generate' });
            }}
          >
            <Sparkles aria-hidden="true" />
            {t('keys.generate.button')}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setDialog({ kind: 'import' });
            }}
          >
            <Upload aria-hidden="true" />
            {t('keys.import.button')}
          </Button>
        </div>
        {snapshot.keys.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('keys.empty')}</p>
        ) : (
          <ul aria-label={t('keys.title')} className="flex flex-col gap-1">
            {snapshot.keys.map((key) => (
              <li key={key.id} className="flex items-center gap-3 rounded-md border px-3 py-2">
                <KeyRound aria-hidden="true" className="size-4 shrink-0 opacity-70" />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-sm font-medium">{key.name}</span>
                  <span className="truncate font-mono text-xs text-muted-foreground">
                    {key.algorithm} {key.fingerprint}
                  </span>
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void copyPublicKey(key)}
                  aria-label={t('keys.copyPublicFor', { name: key.name })}
                >
                  <Copy aria-hidden="true" />
                  {copied === key.id ? t('keys.copied') : t('keys.copyPublic')}
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('keys.exportFor', { name: key.name })}
                  title={exported === key.id ? t('keys.exported') : t('keys.export')}
                  onClick={() => void exportPublicKey(key)}
                >
                  <Download aria-hidden="true" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('keys.deployFor', { name: key.name })}
                  title={t('keys.deploy.button')}
                  onClick={() => {
                    setDialog({ kind: 'deploy', key });
                  }}
                >
                  <Send aria-hidden="true" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('keys.rename', { name: key.name })}
                  title={t('common.rename')}
                  onClick={() => {
                    setDialog({ kind: 'renameKey', key });
                  }}
                >
                  <Pencil aria-hidden="true" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-8"
                  aria-label={t('keys.delete', { name: key.name })}
                  title={t('common.delete')}
                  onClick={() => {
                    setDialog({ kind: 'deleteKey', key });
                  }}
                >
                  <Trash2 aria-hidden="true" />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="identities-title" className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <h2 id="identities-title" className="text-base font-semibold">
            {t('identities.title')}
          </h2>
          <Button
            size="sm"
            className="ml-auto"
            onClick={() => {
              setDialog({ kind: 'identity', identity: null });
            }}
          >
            <Plus aria-hidden="true" />
            {t('identities.new')}
          </Button>
        </div>
        {snapshot.identities.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('identities.empty')}</p>
        ) : (
          <ul aria-label={t('identities.title')} className="flex flex-col gap-1">
            {snapshot.identities.map((identity) => {
              const key = snapshot.keys.find((k) => k.id === identity.keyId);
              const methods = [
                key ? t('identities.withKey', { name: key.name }) : null,
                identity.hasPassword ? t('identities.withPassword') : null,
              ].filter(Boolean);
              return (
                <li
                  key={identity.id}
                  className="flex items-center gap-3 rounded-md border px-3 py-2"
                >
                  <UserRound aria-hidden="true" className="size-4 shrink-0 opacity-70" />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-sm font-medium">{identity.name}</span>
                    <span className="truncate text-xs text-muted-foreground">
                      {identity.username} · {methods.join(' + ')}
                    </span>
                  </span>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-8"
                    aria-label={t('identities.edit', { name: identity.name })}
                    title={t('common.edit')}
                    onClick={() => {
                      setDialog({ kind: 'identity', identity });
                    }}
                  >
                    <Pencil aria-hidden="true" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-8"
                    aria-label={t('identities.delete', { name: identity.name })}
                    title={t('common.delete')}
                    onClick={() => {
                      setDialog({ kind: 'deleteIdentity', identity });
                    }}
                  >
                    <Trash2 aria-hidden="true" />
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <KnownHostsList />

      {dialog?.kind === 'generate' && (
        <KeyGenerateDialog onGenerated={() => void refresh()} onClose={close} />
      )}
      {dialog?.kind === 'deploy' && (
        <KeyDeployDialog
          keyEntry={dialog.key}
          // Déploiement de clé : hôtes SSH seulement (un hôte RDP est refusé par le cœur).
          hosts={snapshot.hosts.filter((h) => h.protocol.kind === 'ssh')}
          onClose={close}
          onManageHostKeys={() => {
            document.getElementById('known-hosts-title')?.scrollIntoView();
          }}
        />
      )}
      {dialog?.kind === 'import' && (
        <KeyImportDialog onImported={() => void refresh()} onClose={close} />
      )}
      {dialog?.kind === 'renameKey' && (
        <NameDialog
          title={t('keys.renameTitle')}
          label={t('keys.name')}
          initial={dialog.key.name}
          onSave={async (name) => {
            await keyRename(dialog.key.id, name);
            await refresh();
          }}
          onClose={close}
        />
      )}
      {dialog?.kind === 'deleteKey' && (
        <ConfirmDialog
          title={t('keys.deleteTitle')}
          message={t('keys.deleteMessage', { name: dialog.key.name })}
          confirmLabel={t('common.delete')}
          destructive
          onCancel={close}
          onConfirm={() => {
            const target = dialog.key;
            close();
            void run(() => keyDelete(target.id));
          }}
        />
      )}
      {dialog?.kind === 'identity' && (
        <IdentityForm
          identity={dialog.identity}
          keys={snapshot.keys}
          onSaved={() => void refresh()}
          onClose={close}
        />
      )}
      {dialog?.kind === 'deleteIdentity' && (
        <ConfirmDialog
          title={t('identities.deleteTitle')}
          message={t('identities.deleteMessage', { name: dialog.identity.name })}
          confirmLabel={t('common.delete')}
          destructive
          onCancel={close}
          onConfirm={() => {
            const target = dialog.identity;
            close();
            void run(() => identityDelete(target.id));
          }}
        />
      )}
    </div>
  );
}
