import { Columns2, PanelRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';

import { Button } from '@/components/ui/button';
import { defaultTabTitle, useSessions } from '@/stores/sessions';

/** Section SFTP de la barre latérale : ouvre l'explorateur sur une session connectée. */
export function SftpSectionView() {
  const { t } = useTranslation();
  const tabs = useSessions(
    useShallow((s) =>
      s.tabs.filter(
        (tab) => tab.kind === 'terminal' && tab.local === null && tab.status === 'connected',
      ),
    ),
  );

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-6">
      <h1 className="text-xl font-semibold">{t('nav.sftp')}</h1>
      <p className="text-sm text-muted-foreground">{t('sftp.sectionIntro')}</p>
      {tabs.length === 0 ? (
        <p className="rounded-md border p-4 text-sm text-muted-foreground">{t('sftp.noSession')}</p>
      ) : (
        <ul className="flex flex-col divide-y rounded-md border">
          {tabs.map((tab) => (
            <li key={tab.id} className="flex flex-wrap items-center gap-2 p-3">
              <span className="min-w-0 flex-1 truncate font-medium">
                {defaultTabTitle(tab) ?? t('tabs.newSession')}
              </span>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  const sessions = useSessions.getState();
                  sessions.update(tab.id, { sftpPanelOpen: true });
                  sessions.activate(tab.id);
                }}
              >
                <PanelRight aria-hidden="true" />
                {t('sftp.openPanel')}
              </Button>
              <Button
                size="sm"
                onClick={() => {
                  useSessions.getState().openSftpTab(tab.id);
                }}
              >
                <Columns2 aria-hidden="true" />
                {t('sftp.openDual')}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
