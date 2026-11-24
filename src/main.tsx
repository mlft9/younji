import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { installContextMenuGuard } from './lib/contextMenu';
import { installExternalDropGuard } from './features/sftp/dnd';
import { initI18n } from './i18n';
import { detectLanguage, systemLanguages } from './i18n/languages';
import { loadSettings, useSettings } from './stores/settings';
import './index.css';

async function start() {
  // Fichiers déposés depuis le système : ignorés (seul le glisser-déposer interne est géré).
  installExternalDropGuard();
  // Menu natif de la WebView (Précédent, Recharger…) : bloqué hors des champs de saisie.
  installContextMenuGuard();
  // Paramètres lus dans settings.json (géré par le cœur) avant le premier rendu : la langue est
  // fixée tout de suite, pour éviter un affichage dans la mauvaise langue.
  await loadSettings();
  const preference = useSettings.getState().language;
  initI18n(preference === 'system' ? detectLanguage(systemLanguages()) : preference);

  const container = document.getElementById('root');
  if (container) {
    createRoot(container).render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  }
}

void start();
