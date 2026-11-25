import { LoaderCircle, RotateCw, Terminal } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { ErrorMessage } from '@/features/connect/ErrorMessage';
import { termProfiles, toAppError } from '@/lib/ipc';
import type { LocalProfile } from '@/lib/types';

import { openLocalTerminal } from './openLocal';

/**
 * Section « Terminal local » (LOC-01, LOC-02) : shells détectés par le cœur. Seuls ces profils
 * peuvent être ouverts ; l'interface ne connaît que leur nom et leur identifiant.
 */
export function LocalTerminalsView() {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<LocalProfile[] | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await termProfiles();
      setProfiles(list);
      setErrorCode(null);
    } catch (error) {
      setProfiles([]);
      setErrorCode(toAppError(error).code);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    termProfiles()
      .then((list) => {
        if (!cancelled) setProfiles(list);
      })
      .catch((failure: unknown) => {
        if (!cancelled) {
          setProfiles([]);
          setErrorCode(toAppError(failure).code);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-6">
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-xl font-semibold">{t('nav.local')}</h1>
        <Button
          size="sm"
          variant="outline"
          disabled={profiles === null}
          onClick={() => {
            setProfiles(null);
            setErrorCode(null);
            void load();
          }}
        >
          <RotateCw aria-hidden="true" />
          {t('local.refresh')}
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">{t('local.intro')}</p>
      {errorCode && <ErrorMessage code={errorCode} />}
      {profiles === null ? (
        <p role="status" className="flex items-center gap-2 text-sm text-muted-foreground">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          {t('local.detecting')}
        </p>
      ) : profiles.length === 0 ? (
        !errorCode && (
          <p className="rounded-md border p-4 text-sm text-muted-foreground">{t('local.none')}</p>
        )
      ) : (
        <ul className="flex flex-col divide-y rounded-md border">
          {profiles.map((profile) => (
            <li key={profile.id} className="flex flex-wrap items-center gap-2 p-3">
              <Terminal aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate">
                <span className="font-medium">{profile.name}</span>
                <span className="ml-2 text-xs text-muted-foreground">
                  {t(`local.kind.${profile.kind}`)}
                  {profile.isDefault && ` · ${t('local.default')}`}
                </span>
              </span>
              <Button
                size="sm"
                onClick={() => {
                  openLocalTerminal(profile);
                }}
              >
                {t('local.open')}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
