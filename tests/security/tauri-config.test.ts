// @vitest-environment node
//
// Garde-fou de durcissement : ces tests échouent si la configuration Tauri est affaiblie
// (CSP, API globale, capabilities, isolation). Toute modification volontaire de ces règles
// passe par une décision du porteur et un ADR, jamais par une simple retouche de ce test.
import { readdirSync, readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { loadIsolation } from './load-isolation';

const root = new URL('../../', import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), 'utf8');

type Csp = Record<string, string>;

interface TauriConfig {
  identifier: string;
  productName: string;
  app: {
    withGlobalTauri?: boolean;
    windows: { url?: string }[];
    security: {
      csp: Csp;
      devCsp: Csp;
      freezePrototype?: boolean;
      capabilities?: string[];
      assetProtocol?: { enable?: boolean };
      dangerousDisableAssetCspModification?: unknown;
      pattern?: { use: string; options?: { dir?: string } };
    };
  };
}

interface Capability {
  identifier: string;
  windows: string[];
  permissions: (string | { identifier: string })[];
  remote?: unknown;
}

const config = JSON.parse(read('src-tauri/tauri.conf.json')) as TauriConfig;
const security = config.app.security;

const capabilities: Capability[] = readdirSync(new URL('src-tauri/capabilities/', root))
  .filter((file) => file.endsWith('.json'))
  .map((file) => JSON.parse(read(`src-tauri/capabilities/${file}`)) as Capability);

const permissionIds = capabilities.flatMap((c) =>
  c.permissions.map((p) => (typeof p === 'string' ? p : p.identifier)),
);

/** Commandes déclarées dans build.rs (`AppManifest::commands(&[...])`). */
function buildRsCommands(): string[] {
  const match = /\.commands\(&\[([^\]]*)\]\)/.exec(read('src-tauri/build.rs'));
  expect(match, 'liste des commandes introuvable dans build.rs').not.toBeNull();
  return [...(match?.[1] ?? '').matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1] ?? '');
}

function sources(csp: Csp, directive: string): string[] {
  return (csp[directive] ?? '').split(/\s+/).filter(Boolean);
}

describe.each([
  ['CSP de production', security.csp],
  ['CSP de développement', security.devCsp],
])('%s', (_, csp) => {
  it('limite tout au contenu embarqué par défaut', () => {
    expect(sources(csp, 'default-src')).toEqual(["'self'"]);
    expect(sources(csp, 'object-src')).toEqual(["'none'"]);
    expect(sources(csp, 'base-uri')).toEqual(["'none'"]);
    expect(sources(csp, 'frame-ancestors')).toEqual(["'none'"]);
  });

  it("n'autorise ni eval ni script en ligne", () => {
    const all = Object.values(csp).join(' ');
    expect(all).not.toContain("'unsafe-eval'");
    expect(all).not.toContain("'wasm-unsafe-eval'");
    expect(sources(csp, 'script-src')).toEqual(["'self'"]);
  });

  it("ne charge aucune ressource distante (seule l'IPC locale est joignable)", () => {
    for (const [directive, value] of Object.entries(csp)) {
      for (const source of value.split(/\s+/).filter(Boolean)) {
        // Chaque source non mot-clé n'est admise que dans la directive où elle est utile.
        const allowed =
          source.startsWith("'") ||
          (source === 'data:' && ['img-src', 'font-src'].includes(directive)) ||
          (['ipc:', 'http://ipc.localhost'].includes(source) && directive === 'connect-src') ||
          (csp === security.devCsp &&
            source === 'ws://127.0.0.1:1420' &&
            directive === 'connect-src');
        expect(allowed, `${directive} : source non autorisée « ${source} »`).toBe(true);
      }
    }
  });
});

describe('CSP de production', () => {
  it("n'autorise pas les attributs style en ligne", () => {
    expect(sources(security.csp, 'style-src')).toEqual(["'self'"]);
    expect(security.csp['style-src-attr']).toBeUndefined();
  });

  it('autorise les balises <style> dynamiques de xterm.js, et rien de plus (ADR-018)', () => {
    expect(sources(security.csp, 'style-src-elem')).toEqual(["'self'", "'unsafe-inline'"]);
  });
});

describe.each([
  ['CSP de production', security.csp],
  ['CSP de développement', security.devCsp],
])('%s : styles', (_, csp) => {
  it("n'utilise 'unsafe-inline' que pour les styles", () => {
    for (const directive of Object.keys(csp)) {
      if (sources(csp, directive).includes("'unsafe-inline'")) {
        expect(['style-src', 'style-src-elem'], directive).toContain(directive);
      }
    }
  });
});

describe('configuration de la fenêtre et de la WebView', () => {
  it("n'expose pas l'API Tauri globale et gèle les prototypes", () => {
    expect(config.app.withGlobalTauri).toBe(false);
    expect(security.freezePrototype).toBe(true);
    expect(security.dangerousDisableAssetCspModification).toBeUndefined();
    expect(security.assetProtocol?.enable ?? false).toBe(false);
  });

  it("active le modèle d'isolation (ADR-015)", () => {
    expect(security.pattern).toEqual({ use: 'isolation', options: { dir: '../isolation' } });
  });

  it('crée la seule fenêtre côté Rust, sur le contenu embarqué, avec une garde de navigation', () => {
    // Aucune fenêtre déclarée dans la configuration : la fenêtre principale est créée dans lib.rs.
    expect(config.app.windows).toEqual([]);
    const lib = read('src-tauri/src/lib.rs');
    expect(lib).toContain('WebviewUrl::App(');
    expect(lib).not.toMatch(/WebviewUrl::(External|CustomProtocol)/);
    expect(lib).toContain('.on_navigation(');
    expect(lib).toMatch(/\.on_new_window\([\s\S]*?NewWindowResponse::Deny/);
    // L'étiquette de la fenêtre correspond à la capability.
    const label = /const MAIN_WINDOW: &str = "([^"]+)";/.exec(lib)?.[1];
    expect(capabilities.flatMap((c) => c.windows)).toEqual([label]);
  });

  it("n'embarque pas l'inspecteur web dans la version publiée", () => {
    // Tauri n'active l'inspecteur (clic droit → Inspecter) qu'en compilation de développement,
    // sauf si la fonctionnalité `devtools` est ajoutée : elle donnerait à quiconque devant le
    // poste un accès direct à l'IPC depuis la console, en contournant l'interface.
    const cargo = read('src-tauri/Cargo.toml');
    const tauriDeps = cargo.split('\n').filter((line) => /^\s*tauri\s*=/.test(line));
    expect(tauriDeps).not.toHaveLength(0);
    for (const line of tauriDeps) expect(line).not.toContain('devtools');
    const lib = read('src-tauri/src/lib.rs');
    expect(lib).not.toMatch(/\.devtools\(|open_devtools/);
  });

  it('garde le nom et l’identifiant définis par ADR-014', () => {
    expect(config.productName).toBe('Younji');
    expect(config.identifier).toBe('io.github.mlft9.younji');
  });
});

describe('capabilities', () => {
  it('sont toutes déclarées explicitement dans la configuration', () => {
    expect(security.capabilities?.slice().sort()).toEqual(
      capabilities.map((c) => c.identifier).sort(),
    );
  });

  it("n'accordent aucun accès distant ni plugin sensible", () => {
    for (const capability of capabilities) {
      expect(capability.remote, capability.identifier).toBeUndefined();
      expect(capability.windows.length).toBeGreaterThan(0);
      expect(capability.windows).not.toContain('*');
    }
    for (const id of permissionIds) {
      expect(id, id).not.toMatch(/^(shell|fs|process|http|updater):/);
      expect(id, id).not.toMatch(/:allow-(execute|spawn)$/);
    }
  });

  it('correspondent exactement à la liste attendue (tout ajout se fait délibérément ici)', () => {
    // Liste blanche exacte : une permission ajoutée dans une capability fait échouer ce test
    // tant qu'elle n'est pas ajoutée ici, après revue de sécurité.
    const EXPECTED_PERMISSIONS = [
      'allow-app-info',
      'allow-ssh-connect',
      'allow-ssh-write',
      'allow-ssh-resize',
      'allow-ssh-ack',
      'allow-ssh-host-key-decision',
      'allow-ssh-close',
      'allow-ssh-pick-key-file',
      'allow-open-external-url',
      'allow-ssh-connect-host',
      'allow-vault-status',
      'allow-vault-create',
      'allow-vault-unlock',
      'allow-vault-lock',
      'allow-vault-change-password',
      'allow-vault-touch',
      'allow-vault-watch',
      'allow-vault-set-auto-lock',
      'allow-vault-snapshot',
      'allow-host-save',
      'allow-host-delete',
      'allow-group-save',
      'allow-group-delete',
      'allow-identity-save',
      'allow-identity-delete',
      'allow-key-import',
      'allow-key-rename',
      'allow-key-delete',
      'allow-known-host-list',
      'allow-known-host-delete',
      'allow-settings-get',
      'allow-settings-set',
      // Étape 3 : 2FA, génération, export et déploiement de clés.
      'allow-ssh-auth-answer',
      'allow-key-generate',
      'allow-key-export-public',
      'allow-key-deploy',
      'allow-sftp-open',
      'allow-sftp-close',
      'allow-sftp-home',
      'allow-sftp-list',
      'allow-sftp-mkdir',
      'allow-sftp-rename',
      'allow-sftp-remove',
      'allow-sftp-chmod',
      'allow-sftp-download',
      'allow-sftp-upload',
      'allow-sftp-transfer-cancel',
      'allow-sftp-conflict-decision',
      'allow-sftp-edit',
      'allow-sftp-edit-close',
      'allow-sftp-edit-force',
      'allow-local-pick-root',
      'allow-local-list',
      // Étape 5 : snippets du coffre (SNP-01).
      'allow-snippet-save',
      'allow-snippet-delete',
      // Étape 7 : terminal local (profils détectés par le cœur, désignés par identifiant).
      'allow-term-profiles',
      'allow-term-open',
      'allow-term-write',
      'allow-term-resize',
      'allow-term-ack',
      'allow-term-close',
    ];
    expect([...permissionIds].sort()).toEqual([...EXPECTED_PERMISSIONS].sort());
  });

  it('accordent uniquement des commandes de la liste blanche de l’isolation', () => {
    const allowed = loadIsolation().__YOUNJI_ISOLATION__.allowedCommands;
    const appPermissions = permissionIds.filter((id) => !id.includes(':'));
    for (const id of appPermissions) {
      expect(id, id).toMatch(/^allow-[a-z0-9-]+$/);
      expect(allowed).toContain(id.slice('allow-'.length).replaceAll('-', '_'));
    }
  });

  it('build.rs, la liste blanche et les capabilities déclarent les mêmes commandes', () => {
    const allowed = [...loadIsolation().__YOUNJI_ISOLATION__.allowedCommands].sort();
    expect(buildRsCommands().sort()).toEqual(allowed);
    const granted = permissionIds
      .filter((id) => id.startsWith('allow-'))
      .map((id) => id.slice('allow-'.length).replaceAll('-', '_'))
      .sort();
    expect(granted).toEqual(allowed);
  });
});

describe('feuille de style', () => {
  it("masque l'iframe d'isolation sans assouplir la CSP", () => {
    expect(read('src/index.css')).toMatch(
      /#__tauri_isolation__\s*\{\s*display:\s*none !important;/,
    );
  });
});
