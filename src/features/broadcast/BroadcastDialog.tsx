import { Radio } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { SessionChecklist } from '@/components/SessionChecklist';
import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { useBroadcast } from '@/stores/broadcast';
import { useSessions } from '@/stores/sessions';

/** Dialogue d'activation de la diffusion (SSH-08), monté une fois dans AppShell. */
export function BroadcastDialog() {
  const open = useBroadcast((s) => s.dialogOpen);
  // Nouvelle instance à chaque ouverture : la sélection repart de la session active.
  return open ? <BroadcastDialogContent /> : null;
}

function BroadcastDialogContent() {
  const { t } = useTranslation();
  const id = useId();
  const tabs = useSessions((s) => s.tabs);
  const connected = tabs.filter(
    (tab) => tab.kind === 'terminal' && tab.local === null && tab.status === 'connected',
  );
  const current = useBroadcast((s) => s.targets);
  const close = useBroadcast((s) => s.closeDialog);
  const start = useBroadcast((s) => s.start);
  // Aucune cible cochée d'office, sauf la diffusion en cours ou le dernier panneau utilisé.
  const [selected, setSelected] = useState<string[]>(() => {
    const ids = new Set(connected.map((tab) => tab.id));
    if (current.length > 0) return current.filter((t) => ids.has(t));
    const last = useSessions.getState().lastPaneId;
    return last && ids.has(last) ? [last] : [];
  });
  const valid = selected.filter((s) => connected.some((tab) => tab.id === s));

  return (
    <Modal
      labelledBy={`${id}-title`}
      describedBy={`${id}-description`}
      onClose={close}
      scope="window"
    >
      <h2 id={`${id}-title`} className="flex items-center gap-2 text-lg font-semibold">
        <Radio aria-hidden="true" className="size-5 text-red-600" />
        {t('broadcast.title')}
      </h2>
      <p id={`${id}-description`} className="text-sm">
        {t('broadcast.description')}
      </p>
      <SessionChecklist
        sessions={connected}
        selected={valid}
        onChange={setSelected}
        label={t('broadcast.targets')}
      />
      {valid.length >= 2 ? (
        <p
          role="alert"
          className="rounded-md border border-red-600 p-2 text-sm text-red-700 dark:text-red-400"
        >
          {t('broadcast.warning', { count: valid.length })}
        </p>
      ) : (
        <p className="text-sm text-muted-foreground">{t('broadcast.needTwo')}</p>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="outline" onClick={close} data-autofocus>
          {t('common.cancel')}
        </Button>
        <Button
          variant="destructive"
          disabled={valid.length < 2}
          onClick={() => {
            start(valid);
          }}
        >
          {t('broadcast.confirm', { count: valid.length })}
        </Button>
      </div>
    </Modal>
  );
}
