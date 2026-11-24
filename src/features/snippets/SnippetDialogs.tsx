import { useSnippetRun } from '@/stores/snippetRun';

import { SnippetPicker } from './SnippetPicker';
import { SnippetRunDialog } from './SnippetRunDialog';

/** Dialogues d'envoi de snippet, montés une fois dans AppShell. */
export function SnippetDialogs() {
  const pickerTargets = useSnippetRun((s) => s.pickerTargets);
  const run = useSnippetRun((s) => s.run);
  if (run) return <SnippetRunDialog key={run.snippetId} run={run} />;
  if (pickerTargets) return <SnippetPicker targets={pickerTargets} />;
  return null;
}
