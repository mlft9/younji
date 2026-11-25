import { type SyntheticEvent, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { NativeSelect } from '@/components/ui/native-select';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { hostSave, toAppError } from '@/lib/ipc';
import type { EntryId, GroupEntry, HostEntry, HostProtocol, IdentityEntry } from '@/lib/types';

import { parseTags } from './filter';

/** Bornes reprises du cœur (revérifiées par lui). */
const MAX_TAGS = 16;
const MAX_NOTES = 4096;
/** Ports par défaut, proposés au changement de protocole. */
const DEFAULT_PORT = { ssh: 22, rdp: 3389 } as const;
/** Bornes de la résolution RDP, reprises du cœur (revérifiées par lui). */
const RDP_WIDTH = { min: 640, max: 7680 };
const RDP_HEIGHT = { min: 480, max: 4320 };
/** Domaine RDP : mêmes règles que le cœur. */
const RDP_DOMAIN = /^(?![-/])[A-Za-z0-9._-]{0,255}$/;

interface HostFormProps {
  /** Hôte à modifier, ou `null` pour en créer un. */
  host: HostEntry | null;
  /** Groupe proposé pour un nouvel hôte. */
  defaultGroupId: EntryId | null;
  groups: readonly GroupEntry[];
  identities: readonly IdentityEntry[];
  onSaved: () => void;
  onClose: () => void;
}

/** Création ou modification d'un hôte (HST-01). */
export function HostForm({
  host,
  defaultGroupId,
  groups,
  identities,
  onSaved,
  onClose,
}: HostFormProps) {
  const { t } = useTranslation();
  const id = useId();
  const [name, setName] = useState(host?.name ?? '');
  const [address, setAddress] = useState(host?.address ?? '');
  const [port, setPort] = useState(
    String(host?.port ?? DEFAULT_PORT[host?.protocol.kind === 'rdp' ? 'rdp' : 'ssh']),
  );
  const [username, setUsername] = useState(host?.username ?? '');
  const [identityId, setIdentityId] = useState<string>(host?.identityId ?? '');
  const [groupId, setGroupId] = useState<string>(host?.groupId ?? defaultGroupId ?? '');
  const [tags, setTags] = useState(host?.tags.join(', ') ?? '');
  const [notes, setNotes] = useState(host?.notes ?? '');
  const initialRdp = host?.protocol.kind === 'rdp' ? host.protocol : null;
  const [protocol, setProtocol] = useState<'ssh' | 'rdp'>(initialRdp ? 'rdp' : 'ssh');
  const [domain, setDomain] = useState(initialRdp?.domain ?? '');
  const [fullscreen, setFullscreen] = useState(
    initialRdp ? initialRdp.resolution.mode === 'fullscreen' : true,
  );
  const [width, setWidth] = useState(
    String(initialRdp?.resolution.mode === 'fixed' ? initialRdp.resolution.width : 1920),
  );
  const [height, setHeight] = useState(
    String(initialRdp?.resolution.mode === 'fixed' ? initialRdp.resolution.height : 1080),
  );

  function changeProtocol(next: 'ssh' | 'rdp') {
    // Port par défaut du protocole, seulement si l'utilisateur n'en a pas saisi d'autre.
    if (port === String(DEFAULT_PORT[protocol])) setPort(String(DEFAULT_PORT[next]));
    setProtocol(next);
  }

  /** Protocole saisi, ou un code d'erreur si les réglages RDP sont hors bornes. */
  function protocolValue(): HostProtocol | string {
    if (protocol === 'ssh') return { kind: 'ssh' };
    if (!RDP_DOMAIN.test(domain)) return 'vault.invalid_rdp';
    if (fullscreen) return { kind: 'rdp', domain, resolution: { mode: 'fullscreen' } };
    const w = Number(width);
    const h = Number(height);
    const inRange =
      /^\d+$/.test(width) &&
      /^\d+$/.test(height) &&
      w >= RDP_WIDTH.min &&
      w <= RDP_WIDTH.max &&
      h >= RDP_HEIGHT.min &&
      h <= RDP_HEIGHT.max;
    if (!inRange) return 'vault.invalid_rdp';
    return { kind: 'rdp', domain, resolution: { mode: 'fixed', width: w, height: h } };
  }
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: SyntheticEvent) {
    event.preventDefault();
    const portNumber = Number(port);
    if (address.trim() === '') {
      setError('vault.invalid_address');
      return;
    }
    if (!/^\d+$/.test(port) || portNumber < 1 || portNumber > 65535) {
      setError('vault.invalid_port');
      return;
    }
    if (identityId === '' && username === '') {
      setError('vault.invalid_username');
      return;
    }
    const protocolOrError = protocolValue();
    if (typeof protocolOrError === 'string') {
      setError(protocolOrError);
      return;
    }
    const tagList = parseTags(tags);
    if (tagList.length > MAX_TAGS) {
      setError('vault.invalid_tags');
      return;
    }
    setBusy(true);
    try {
      await hostSave({
        id: host?.id ?? null,
        name: name.trim() === '' ? address.trim() : name.trim(),
        address: address.trim(),
        port: portNumber,
        username,
        identityId: identityId === '' ? null : identityId,
        groupId: groupId === '' ? null : groupId,
        tags: tagList,
        notes,
        protocol: protocolOrError,
      });
      onSaved();
      onClose();
    } catch (failure) {
      setError(toAppError(failure).code);
    } finally {
      setBusy(false);
    }
  }

  const field = (suffix: string) => `${id}-${suffix}`;

  return (
    <Modal labelledBy={field('title')} onClose={onClose} scope="window" className="max-w-xl">
      <form onSubmit={(event) => void submit(event)} noValidate className="flex flex-col gap-4">
        <h2 id={field('title')} className="text-lg font-semibold">
          {host ? t('hosts.form.editTitle') : t('hosts.form.newTitle')}
        </h2>
        {error && <ErrorMessage code={error} />}

        <div className="flex flex-col gap-1.5">
          <label htmlFor={field('name')} className="text-sm font-medium">
            {t('hosts.form.name')}
          </label>
          <Input
            id={field('name')}
            value={name}
            onChange={(e) => {
              setName(e.target.value);
            }}
            placeholder={t('hosts.form.namePlaceholder')}
            maxLength={128}
            autoComplete="off"
            data-autofocus
          />
        </div>

        <div className="flex flex-col gap-1.5">
          <label htmlFor={field('protocol')} className="text-sm font-medium">
            {t('hosts.form.protocol')}
          </label>
          <NativeSelect
            id={field('protocol')}
            value={protocol}
            onChange={(e) => {
              changeProtocol(e.target.value === 'rdp' ? 'rdp' : 'ssh');
            }}
          >
            <option value="ssh">{t('hosts.form.protocolSsh')}</option>
            <option value="rdp">{t('hosts.form.protocolRdp')}</option>
          </NativeSelect>
        </div>

        <div className="grid grid-cols-[1fr_6rem] gap-3">
          <div className="flex flex-col gap-1.5">
            <label htmlFor={field('address')} className="text-sm font-medium">
              {t('hosts.form.address')}
            </label>
            <Input
              id={field('address')}
              value={address}
              onChange={(e) => {
                setAddress(e.target.value);
              }}
              placeholder={t('connect.hostPlaceholder')}
              maxLength={255}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor={field('port')} className="text-sm font-medium">
              {t('connect.port')}
            </label>
            <Input
              id={field('port')}
              value={port}
              onChange={(e) => {
                setPort(e.target.value);
              }}
              inputMode="numeric"
              maxLength={5}
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-1.5">
            <label htmlFor={field('identity')} className="text-sm font-medium">
              {t('hosts.form.identity')}
            </label>
            <NativeSelect
              id={field('identity')}
              value={identityId}
              onChange={(e) => {
                setIdentityId(e.target.value);
              }}
            >
              <option value="">{t('hosts.form.noIdentity')}</option>
              {identities.map((identity) => (
                <option key={identity.id} value={identity.id}>
                  {identity.name}
                </option>
              ))}
            </NativeSelect>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor={field('user')} className="text-sm font-medium">
              {t('connect.username')}
            </label>
            <Input
              id={field('user')}
              value={username}
              onChange={(e) => {
                setUsername(e.target.value);
              }}
              placeholder={identityId === '' ? '' : t('hosts.form.usernameFromIdentity')}
              maxLength={256}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
        </div>
        {identityId === '' && (
          <p className="-mt-2 text-xs text-muted-foreground">
            {t(protocol === 'rdp' ? 'hosts.form.rdpNoIdentityHint' : 'hosts.form.noIdentityHint')}
          </p>
        )}

        {protocol === 'rdp' && (
          <fieldset className="flex flex-col gap-3 rounded-md border p-3">
            <legend className="px-1 text-sm font-medium">{t('hosts.form.rdpSettings')}</legend>
            <div className="flex flex-col gap-1.5">
              <label htmlFor={field('domain')} className="text-sm font-medium">
                {t('hosts.form.domain')}
              </label>
              <Input
                id={field('domain')}
                value={domain}
                onChange={(e) => {
                  setDomain(e.target.value);
                }}
                placeholder={t('hosts.form.domainPlaceholder')}
                maxLength={255}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={fullscreen}
                onChange={(e) => {
                  setFullscreen(e.target.checked);
                }}
              />
              {t('hosts.form.fullscreen')}
            </label>
            {!fullscreen && (
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1.5">
                  <label htmlFor={field('width')} className="text-sm font-medium">
                    {t('hosts.form.width')}
                  </label>
                  <Input
                    id={field('width')}
                    value={width}
                    onChange={(e) => {
                      setWidth(e.target.value);
                    }}
                    inputMode="numeric"
                    maxLength={4}
                  />
                </div>
                <div className="flex flex-col gap-1.5">
                  <label htmlFor={field('height')} className="text-sm font-medium">
                    {t('hosts.form.height')}
                  </label>
                  <Input
                    id={field('height')}
                    value={height}
                    onChange={(e) => {
                      setHeight(e.target.value);
                    }}
                    inputMode="numeric"
                    maxLength={4}
                  />
                </div>
              </div>
            )}
          </fieldset>
        )}

        <div className="flex flex-col gap-1.5">
          <label htmlFor={field('group')} className="text-sm font-medium">
            {t('hosts.form.group')}
          </label>
          <NativeSelect
            id={field('group')}
            value={groupId}
            onChange={(e) => {
              setGroupId(e.target.value);
            }}
          >
            <option value="">{t('hosts.ungrouped')}</option>
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.name}
              </option>
            ))}
          </NativeSelect>
        </div>

        <div className="flex flex-col gap-1.5">
          <label htmlFor={field('tags')} className="text-sm font-medium">
            {t('hosts.form.tags')}
          </label>
          <Input
            id={field('tags')}
            value={tags}
            onChange={(e) => {
              setTags(e.target.value);
            }}
            placeholder={t('hosts.form.tagsPlaceholder')}
            maxLength={MAX_TAGS * 34}
            autoComplete="off"
          />
        </div>

        <div className="flex flex-col gap-1.5">
          <label htmlFor={field('notes')} className="text-sm font-medium">
            {t('hosts.form.notes')}
          </label>
          <textarea
            id={field('notes')}
            value={notes}
            onChange={(e) => {
              setNotes(e.target.value);
            }}
            maxLength={MAX_NOTES}
            rows={3}
            className="min-h-16 rounded-md border border-input bg-background px-3 py-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
        </div>

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
