import { TriangleAlert } from 'lucide-react';
import { type SyntheticEvent, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { NativeSelect } from '@/components/ui/native-select';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { toAppError, tunnelSave } from '@/lib/ipc';
import type { BindScope, HostEntry, TunnelEntry, TunnelKind } from '@/lib/types';

import { hasDestination, isValidDestination, isValidPort, openSshFlag } from './describe';

interface TunnelFormProps {
  tunnel: TunnelEntry | null;
  hosts: readonly HostEntry[];
  onSaved: () => void;
  onClose: () => void;
}

const KINDS: readonly TunnelKind[] = ['local', 'remote', 'dynamic'];

/** Port saisi : entier, ou `NaN` si le champ n'en contient pas un. */
function parsePort(value: string): number {
  return /^\d{1,5}$/.test(value.trim()) ? Number(value.trim()) : Number.NaN;
}

/**
 * Création ou modification d'un tunnel (TUN-01 à TUN-03). L'écoute se fait sur `127.0.0.1` par
 * défaut ; choisir `0.0.0.0` affiche un avertissement et exige de cocher une confirmation
 * explicite, que le cœur vérifie aussi à l'enregistrement.
 */
export function TunnelForm({ tunnel, hosts, onSaved, onClose }: TunnelFormProps) {
  const { t } = useTranslation();
  const id = useId();
  const [name, setName] = useState(tunnel?.name ?? '');
  const [hostId, setHostId] = useState(tunnel?.hostId ?? hosts[0]?.id ?? '');
  const [kind, setKind] = useState<TunnelKind>(tunnel?.kind ?? 'local');
  const [bindScope, setBindScope] = useState<BindScope>(tunnel?.bindScope ?? 'loopback');
  const [bindPort, setBindPort] = useState(tunnel ? String(tunnel.bindPort) : '');
  const [destHost, setDestHost] = useState(
    tunnel && tunnel.destHost !== '' ? tunnel.destHost : '127.0.0.1',
  );
  const [destPort, setDestPort] = useState(
    tunnel && tunnel.destPort !== 0 ? String(tunnel.destPort) : '',
  );
  const [autoStart, setAutoStart] = useState(tunnel?.autoStart ?? false);
  // Jamais pré-cochée, même pour un tunnel déjà exposé : chaque enregistrement la redemande.
  const [exposureConfirmed, setExposureConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const exposed = bindScope === 'allInterfaces';
  const withDestination = hasDestination(kind);
  const preview = openSshFlag({
    kind,
    bindScope,
    bindPort: parsePort(bindPort) || 0,
    destHost: withDestination ? destHost.trim() : '',
    destPort: withDestination ? parsePort(destPort) || 0 : 0,
  });

  async function submit(event: SyntheticEvent) {
    event.preventDefault();
    const listen = parsePort(bindPort);
    const target = parsePort(destPort);
    if (name.trim() === '') {
      setError('vault.invalid_name');
      return;
    }
    if (hostId === '') {
      setError('tunnels.no_host');
      return;
    }
    if (
      !isValidPort(listen) ||
      (withDestination && (!isValidPort(target) || !isValidDestination(destHost.trim())))
    ) {
      setError('vault.invalid_tunnel');
      return;
    }
    if (exposed && !exposureConfirmed) {
      setError('vault.tunnel_exposure_unconfirmed');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await tunnelSave({
        id: tunnel?.id ?? null,
        name,
        hostId,
        kind,
        bindScope,
        bindPort: listen,
        destHost: withDestination ? destHost.trim() : '',
        destPort: withDestination ? target : 0,
        autoStart,
        exposureConfirmed: exposed && exposureConfirmed,
      });
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
          {tunnel ? t('tunnels.editTitle') : t('tunnels.newTitle')}
        </h2>
        {error && <ErrorMessage code={error} />}
        <label className="flex flex-col gap-1 text-sm">
          {t('tunnels.name')}
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
          {t('tunnels.host')}
          <NativeSelect
            value={hostId}
            onChange={(e) => {
              setHostId(e.target.value);
            }}
          >
            {hosts.map((h) => (
              <option key={h.id} value={h.id}>
                {h.name} ({h.address})
              </option>
            ))}
          </NativeSelect>
        </label>
        <fieldset className="flex flex-col gap-2 text-sm">
          <legend className="mb-1 font-medium">{t('tunnels.kind')}</legend>
          {KINDS.map((k) => (
            <label key={k} className="flex items-start gap-2">
              <input
                type="radio"
                name={`${id}-kind`}
                value={k}
                checked={kind === k}
                onChange={() => {
                  setKind(k);
                }}
                className="mt-0.5 size-4"
              />
              <span>
                <span className="font-medium">{t(`tunnels.kinds.${k}`)}</span>
                <span className="block text-xs text-muted-foreground">
                  {t(`tunnels.kindHelp.${k}`)}
                </span>
              </span>
            </label>
          ))}
        </fieldset>
        <div className="grid gap-4 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-sm">
            {kind === 'remote' ? t('tunnels.listenRemote') : t('tunnels.listenLocal')}
            <NativeSelect
              value={bindScope}
              onChange={(e) => {
                setBindScope(e.target.value === 'allInterfaces' ? 'allInterfaces' : 'loopback');
                setExposureConfirmed(false);
              }}
            >
              <option value="loopback">{t('tunnels.scopes.loopback')}</option>
              <option value="allInterfaces">{t('tunnels.scopes.allInterfaces')}</option>
            </NativeSelect>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            {t('tunnels.bindPort')}
            <Input
              inputMode="numeric"
              value={bindPort}
              maxLength={5}
              placeholder={kind === 'dynamic' ? '1080' : '8080'}
              onChange={(e) => {
                setBindPort(e.target.value);
              }}
            />
          </label>
        </div>
        {exposed && (
          <div
            role="alert"
            className="flex flex-col gap-2 rounded-md border border-amber-500/60 bg-amber-500/10 p-3 text-sm"
          >
            <p className="flex gap-2">
              <TriangleAlert aria-hidden="true" className="size-5 shrink-0 text-amber-600" />
              <span>
                {t(kind === 'remote' ? 'tunnels.exposureRemote' : 'tunnels.exposureLocal')}
                {kind === 'dynamic' && ` ${t('tunnels.exposureSocks')}`}
              </span>
            </p>
            <label className="flex items-center gap-2 font-medium">
              <input
                type="checkbox"
                checked={exposureConfirmed}
                onChange={(e) => {
                  setExposureConfirmed(e.target.checked);
                }}
                className="size-4"
              />
              {t('tunnels.exposureConfirm')}
            </label>
          </div>
        )}
        {withDestination && (
          <div className="grid gap-4 sm:grid-cols-2">
            <label className="flex flex-col gap-1 text-sm">
              {kind === 'remote' ? t('tunnels.destLocal') : t('tunnels.destRemote')}
              <Input
                value={destHost}
                maxLength={255}
                spellCheck={false}
                onChange={(e) => {
                  setDestHost(e.target.value);
                }}
              />
            </label>
            <label className="flex flex-col gap-1 text-sm">
              {t('tunnels.destPort')}
              <Input
                inputMode="numeric"
                value={destPort}
                maxLength={5}
                placeholder="80"
                onChange={(e) => {
                  setDestPort(e.target.value);
                }}
              />
            </label>
          </div>
        )}
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(e) => {
              setAutoStart(e.target.checked);
            }}
            className="size-4"
          />
          {t('tunnels.autoStart')}
        </label>
        <p className="text-xs text-muted-foreground">
          {t('tunnels.equivalent')} <code className="font-mono">{preview}</code>
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={busy || (exposed && !exposureConfirmed)}>
            {t('common.save')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
