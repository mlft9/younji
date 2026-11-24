import { useTranslation } from 'react-i18next';

import { sessionAddress } from '@/lib/sessionAddress';
import { defaultTabTitle, type SessionTab } from '@/stores/sessions';

interface SessionChecklistProps {
  /** Sessions proposées (terminaux connectés). */
  sessions: SessionTab[];
  selected: string[];
  onChange: (selected: string[]) => void;
  /** Libellé accessible de la liste. */
  label: string;
}

/**
 * Liste de sessions à cocher (diffusion, envoi de snippet). Chaque ligne montre le nom **et**
 * l'adresse complète, pour qu'on vérifie les serveurs visés avant d'agir (M7).
 */
export function SessionChecklist({ sessions, selected, onChange, label }: SessionChecklistProps) {
  const { t } = useTranslation();
  if (sessions.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('sessionsPicker.none')}</p>;
  }
  return (
    <fieldset className="flex max-h-64 flex-col gap-1 overflow-auto rounded-md border p-2">
      <legend className="sr-only">{label}</legend>
      {sessions.map((tab) => {
        const checked = selected.includes(tab.id);
        const title = defaultTabTitle(tab) ?? t('tabs.newSession');
        const address = sessionAddress(tab);
        return (
          <label
            key={tab.id}
            className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-sm hover:bg-accent"
          >
            <input
              type="checkbox"
              checked={checked}
              onChange={() => {
                onChange(checked ? selected.filter((id) => id !== tab.id) : [...selected, tab.id]);
              }}
            />
            <span className="font-medium">{title}</span>
            {address !== '' && address !== title && (
              <span className="truncate font-mono text-xs text-muted-foreground">{address}</span>
            )}
          </label>
        );
      })}
    </fieldset>
  );
}
