import { Radio, Square } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { useBroadcast } from '@/stores/broadcast';

/**
 * Bandeau rouge en haut de la fenêtre, visible depuis tous les onglets tant que la diffusion
 * est active (SSH-08) : nombre de cibles et arrêt en un clic.
 */
export function BroadcastBanner() {
  const { t } = useTranslation();
  const count = useBroadcast((s) => (s.targets.length >= 2 ? s.targets.length : 0));
  const stop = useBroadcast((s) => s.stop);
  if (count === 0) return null;
  return (
    <div
      role="status"
      aria-live="assertive"
      className="flex shrink-0 items-center justify-between gap-3 bg-red-600 px-3 py-1.5 text-sm font-semibold text-white"
    >
      <span className="flex items-center gap-2">
        <Radio aria-hidden="true" className="size-4 animate-pulse" />
        {t('broadcast.banner', { count })}
      </span>
      <Button
        size="sm"
        variant="outline"
        className="h-7 border-white bg-white text-red-700 hover:bg-red-50 hover:text-red-800"
        onClick={stop}
      >
        <Square aria-hidden="true" />
        {t('broadcast.stop')}
      </Button>
    </div>
  );
}
