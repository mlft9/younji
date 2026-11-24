import { ClipboardPaste } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { sessionAddress } from '@/lib/sessionAddress';
import { defaultTabTitle, type SessionTab } from '@/stores/sessions';

import { type PasteAnalysis, pastePreview } from './paste';

interface PasteConfirmDialogProps {
  analysis: PasteAnalysis;
  /** Sessions qui recevront le collage si la diffusion est active (SSH-08), sinon vide. */
  broadcastTo?: SessionTab[];
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmation d'un collage risqué (plusieurs lignes ou caractères de contrôle), avec un
 * aperçu où chaque caractère invisible est rendu visible. Le focus part sur « Annuler ».
 */
export function PasteConfirmDialog({
  analysis,
  broadcastTo = [],
  onConfirm,
  onCancel,
}: PasteConfirmDialogProps) {
  const { t } = useTranslation();
  const id = useId();
  const { preview, truncated } = pastePreview(analysis.text);

  return (
    <Modal labelledBy={`${id}-title`} describedBy={`${id}-description`} onClose={onCancel}>
      <h2 id={`${id}-title`} className="flex items-center gap-2 text-lg font-semibold">
        <ClipboardPaste aria-hidden="true" className="size-5" />
        {t('paste.title')}
      </h2>
      <div id={`${id}-description`} className="flex flex-col gap-2 text-sm">
        <p>{t('paste.lines', { count: analysis.lineCount })}</p>
        {broadcastTo.length > 0 && (
          <div
            role="alert"
            className="rounded-md border-2 border-red-600 p-2 font-medium text-red-700 dark:text-red-400"
          >
            <p>{t('paste.broadcast', { count: broadcastTo.length })}</p>
            <ul className="mt-1 list-disc pl-5 font-normal">
              {broadcastTo.map((tab) => (
                <li key={tab.id}>
                  {defaultTabTitle(tab) ?? t('tabs.newSession')}{' '}
                  <span className="font-mono text-xs">{sessionAddress(tab)}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
        {analysis.hasControlCharacters && (
          <p className="font-medium text-destructive">{t('paste.controlCharacters')}</p>
        )}
      </div>
      <pre
        aria-label={t('paste.preview')}
        className="max-h-64 overflow-auto rounded-md border bg-muted/50 p-3 font-mono text-xs whitespace-pre-wrap break-all"
      >
        {preview}
      </pre>
      {truncated && <p className="text-xs text-muted-foreground">{t('paste.truncated')}</p>}
      <div className="flex justify-end gap-2">
        <Button variant="outline" data-autofocus onClick={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button variant={broadcastTo.length > 0 ? 'destructive' : 'default'} onClick={onConfirm}>
          {broadcastTo.length > 0
            ? t('paste.confirmBroadcast', { count: broadcastTo.length })
            : t('paste.confirm')}
        </Button>
      </div>
    </Modal>
  );
}
