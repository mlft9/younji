//! Données du coffre (HST-01 à HST-03, KEY-02, HKY-04) et règles de validation.
//!
//! Tout ce qui est ici est chiffré sur le disque. Les secrets (mot de passe d'une identité,
//! clé privée importée) sont gardés en `SecretString` : ils ne sont jamais affichés par `Debug`
//! et ne sortent du coffre que vers le cœur SSH, jamais vers l'interface.
//!
//! Chaque modification passe par une méthode de [`VaultData`] qui valide les champs (longueurs,
//! caractères de contrôle, nombre d'entrées) et l'intégrité des références (une identité ou un
//! groupe désigné doit exister).

use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::VaultError;

/// Version du schéma des données (indépendante de la version du format de fichier).
pub const SCHEMA_VERSION: u32 = 1;

/// Longueur maximale d'un nom (hôte, groupe, identité, clé), en caractères.
pub const MAX_NAME_LEN: usize = 128;
/// Longueur maximale d'une adresse d'hôte.
pub const MAX_ADDRESS_LEN: usize = 255;
/// Longueur maximale d'un nom d'utilisateur.
pub const MAX_USERNAME_LEN: usize = 256;
/// Nombre maximal d'étiquettes par hôte.
pub const MAX_TAGS: usize = 16;
/// Longueur maximale d'une étiquette.
pub const MAX_TAG_LEN: usize = 32;
/// Longueur maximale des notes, en caractères.
pub const MAX_NOTES_LEN: usize = 4096;
/// Longueur maximale d'un mot de passe d'identité, en octets.
pub const MAX_SECRET_LEN: usize = 1024;
/// Nombre maximal d'hôtes.
pub const MAX_HOSTS: usize = 10_000;
/// Nombre maximal de groupes, d'identités ou de clés.
pub const MAX_ENTRIES: usize = 1_000;
/// Taille maximale des clés d'hôte connues (texte `known_hosts`).
pub const MAX_KNOWN_HOSTS_LEN: usize = 4 * 1024 * 1024;
/// Nombre maximal de snippets.
pub const MAX_SNIPPETS: usize = 1_000;
/// Taille maximale du contenu d'un snippet, en octets.
pub const MAX_SNIPPET_LEN: usize = 16 * 1024;
/// Longueur maximale de la description d'un snippet, en caractères.
pub const MAX_SNIPPET_DESCRIPTION_LEN: usize = 512;
/// Nombre maximal de variables distinctes `{{nom}}` dans un snippet.
pub const MAX_SNIPPET_VARIABLES: usize = 32;
/// Longueur maximale d'un nom de variable.
pub const MAX_VARIABLE_NAME_LEN: usize = 32;
/// Délais de verrouillage automatique proposés, en minutes. Pas d'option « jamais ».
pub const AUTO_LOCK_CHOICES: [u32; 6] = [1, 5, 10, 15, 30, 60];
/// Délai de verrouillage par défaut, en minutes.
pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 15;

/// Identifiant opaque d'une entrée du coffre.
pub type EntryId = Uuid;

/// Sérialise un secret dans le coffre chiffré (et nulle part ailleurs).
fn serialize_secret<S: Serializer>(secret: &SecretString, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(secret.expose_secret())
}

fn serialize_optional_secret<S: Serializer>(
    secret: &Option<SecretString>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match secret {
        Some(secret) => s.serialize_some(secret.expose_secret()),
        None => s.serialize_none(),
    }
}

/// Hôte du carnet (HST-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Host {
    /// Identifiant.
    pub id: EntryId,
    /// Nom affiché.
    pub name: String,
    /// Nom d'hôte ou adresse IP.
    pub address: String,
    /// Port SSH.
    pub port: u16,
    /// Utilisateur ; vide = celui de l'identité.
    pub username: String,
    /// Identité utilisée pour se connecter.
    pub identity_id: Option<EntryId>,
    /// Groupe (dossier).
    pub group_id: Option<EntryId>,
    /// Étiquettes.
    pub tags: Vec<String>,
    /// Notes libres.
    pub notes: String,
}

/// Groupe d'hôtes, à un seul niveau (HST-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Group {
    /// Identifiant.
    pub id: EntryId,
    /// Nom affiché.
    pub name: String,
}

/// Identité réutilisable : utilisateur + mot de passe et/ou clé (HST-03).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Identity {
    /// Identifiant.
    pub id: EntryId,
    /// Nom affiché.
    pub name: String,
    /// Utilisateur.
    pub username: String,
    /// Mot de passe.
    #[serde(serialize_with = "serialize_optional_secret")]
    pub password: Option<SecretString>,
    /// Clé privée du coffre.
    pub key_id: Option<EntryId>,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("id", &self.id)
            .field("has_password", &self.password.is_some())
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

/// Clé privée importée dans le coffre (KEY-02). Elle est stockée au format OpenSSH, **sans**
/// passphrase : le coffre la protège déjà, et la connexion ne doit rien redemander.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredKey {
    /// Identifiant.
    pub id: EntryId,
    /// Nom affiché.
    pub name: String,
    /// Algorithme (`ssh-ed25519`…).
    pub algorithm: String,
    /// Empreinte `SHA256:…`.
    pub fingerprint: String,
    /// Clé publique au format OpenSSH (une ligne).
    pub public_key: String,
    /// Clé privée au format OpenSSH, non chiffrée (le coffre la chiffre).
    #[serde(serialize_with = "serialize_secret")]
    pub private_key: SecretString,
}

impl fmt::Debug for StoredKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredKey")
            .field("id", &self.id)
            .field("algorithm", &self.algorithm)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// Snippet : commande réutilisable, avec des variables `{{nom}}` demandées à l'envoi (SNP-01,
/// SNP-02). Ce n'est **pas** un secret : il est affiché par l'interface. Il est chiffré au repos
/// avec le reste du coffre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snippet {
    /// Identifiant.
    pub id: EntryId,
    /// Nom affiché.
    pub name: String,
    /// Texte envoyé au terminal (une ou plusieurs lignes).
    pub content: String,
    /// Description libre.
    pub description: String,
}

/// Saisie d'un snippet (création si `id` est `None`).
#[derive(Debug, Clone)]
pub struct SnippetInput {
    /// Snippet à modifier, ou `None` pour en créer un.
    pub id: Option<EntryId>,
    /// Nom affiché.
    pub name: String,
    /// Contenu.
    pub content: String,
    /// Description.
    pub description: String,
}

/// Réglages stockés dans le coffre (authentifiés, donc non modifiables hors de l'application).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultSettings {
    /// Délai de verrouillage automatique, en minutes.
    pub auto_lock_minutes: u32,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES,
        }
    }
}

/// Contenu déchiffré du coffre.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultData {
    schema_version: u32,
    hosts: Vec<Host>,
    groups: Vec<Group>,
    identities: Vec<Identity>,
    keys: Vec<StoredKey>,
    /// Clés d'hôte acceptées, au format `known_hosts` d'OpenSSH (ADR à l'étape 2).
    known_hosts: String,
    settings: VaultSettings,
    /// Snippets (étape 5). Absent des coffres créés avant l'étape 5 : liste vide par défaut.
    #[serde(default)]
    snippets: Vec<Snippet>,
}

impl fmt::Debug for VaultData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultData")
            .field("hosts", &self.hosts.len())
            .field("groups", &self.groups.len())
            .field("identities", &self.identities.len())
            .field("keys", &self.keys.len())
            .field("snippets", &self.snippets.len())
            .finish_non_exhaustive()
    }
}

impl Default for VaultData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            hosts: Vec::new(),
            groups: Vec::new(),
            identities: Vec::new(),
            keys: Vec::new(),
            known_hosts: String::new(),
            settings: VaultSettings::default(),
            snippets: Vec::new(),
        }
    }
}

/// Saisie d'un hôte (création si `id` est `None`).
#[derive(Debug, Clone)]
pub struct HostInput {
    /// Hôte à modifier, ou `None` pour en créer un.
    pub id: Option<EntryId>,
    /// Nom affiché.
    pub name: String,
    /// Adresse (déjà validée comme cible SSH par l'appelant).
    pub address: String,
    /// Port.
    pub port: u16,
    /// Utilisateur (peut être vide si une identité est choisie).
    pub username: String,
    /// Identité.
    pub identity_id: Option<EntryId>,
    /// Groupe.
    pub group_id: Option<EntryId>,
    /// Étiquettes.
    pub tags: Vec<String>,
    /// Notes.
    pub notes: String,
}

/// Changement du mot de passe d'une identité.
pub enum PasswordChange {
    /// Garder le mot de passe actuel.
    Keep,
    /// Retirer le mot de passe.
    Clear,
    /// Remplacer par un nouveau mot de passe.
    Set(SecretString),
}

/// Saisie d'une identité (création si `id` est `None`).
pub struct IdentityInput {
    /// Identité à modifier, ou `None` pour en créer une.
    pub id: Option<EntryId>,
    /// Nom affiché.
    pub name: String,
    /// Utilisateur.
    pub username: String,
    /// Mot de passe.
    pub password: PasswordChange,
    /// Clé du coffre.
    pub key_id: Option<EntryId>,
}

/// Clé à ajouter au coffre (déjà décodée et ré-encodée par l'appelant).
pub struct NewKey {
    /// Nom affiché.
    pub name: String,
    /// Algorithme.
    pub algorithm: String,
    /// Empreinte `SHA256:…`.
    pub fingerprint: String,
    /// Clé publique OpenSSH.
    pub public_key: String,
    /// Clé privée OpenSSH non chiffrée.
    pub private_key: SecretString,
}

fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

/// Nom : 1 à 128 caractères, sans caractère de contrôle, espaces de bord retirés. Publique pour
/// valider un nom avant une opération coûteuse (génération de clé RSA).
pub fn clean_name(name: &str) -> Result<String, VaultError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN || has_control(name) {
        return Err(VaultError::InvalidName);
    }
    Ok(name.to_owned())
}

fn check_username(username: &str, required: bool) -> Result<(), VaultError> {
    let invalid = username.len() > MAX_USERNAME_LEN
        || has_control(username)
        || (required && username.is_empty());
    if invalid {
        Err(VaultError::InvalidUsername)
    } else {
        Ok(())
    }
}

fn clean_tags(tags: &[String]) -> Result<Vec<String>, VaultError> {
    if tags.len() > MAX_TAGS {
        return Err(VaultError::InvalidTags);
    }
    let mut out: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() || tag.chars().count() > MAX_TAG_LEN || has_control(tag) {
            return Err(VaultError::InvalidTags);
        }
        if !out.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            out.push(tag.to_owned());
        }
    }
    Ok(out)
}

fn check_notes(notes: &str) -> Result<(), VaultError> {
    let bad_control = notes
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t');
    if notes.chars().count() > MAX_NOTES_LEN || bad_control {
        Err(VaultError::InvalidNotes)
    } else {
        Ok(())
    }
}

fn check_secret(secret: &SecretString) -> Result<(), VaultError> {
    let len = secret.expose_secret().len();
    if len == 0 || len > MAX_SECRET_LEN {
        Err(VaultError::InvalidSecret)
    } else {
        Ok(())
    }
}

/// Vrai pour un nom de variable admis : `[A-Za-z_][A-Za-z0-9_-]{0,31}`.
pub fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    first_ok
        && name.len() <= MAX_VARIABLE_NAME_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Noms des variables `{{nom}}` d'un contenu, dans l'ordre de première apparition et sans
/// doublon. Un motif `{{…}}` dont le nom n'est pas admis reste du texte littéral. Même règle que
/// `src/features/snippets/variables.ts`.
pub fn snippet_variables(content: &str) -> Vec<&str> {
    let mut names: Vec<&str> = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = &after[..end];
                if is_variable_name(name) {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                    rest = &after[end + 2..];
                } else {
                    // Pas une variable : on reprend juste après la première accolade.
                    rest = &rest[start + 1..];
                }
            }
            None => break,
        }
    }
    names
}

/// Caractères invisibles de mise en forme (largeur nulle, contrôles bidirectionnels, BOM) :
/// ils feraient afficher dans l'aperçu un texte différent de celui qui part (« Trojan Source »).
pub fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// Contenu de snippet : 1 octet à 16 Kio, sans caractère de contrôle hors saut de ligne et
/// tabulation (un `ESC` permettrait de sortir du collage encadré ou de cacher une commande),
/// 32 variables au plus. Les fins de ligne `\r\n` sont ramenées à `\n`.
fn clean_snippet_content(content: &str) -> Result<String, VaultError> {
    let content = content.replace("\r\n", "\n");
    let bad_char = content
        .chars()
        .any(|c| (c.is_control() && c != '\n' && c != '\t') || is_invisible_format(c));
    if content.trim().is_empty() || content.len() > MAX_SNIPPET_LEN || bad_char {
        return Err(VaultError::InvalidSnippet);
    }
    if snippet_variables(&content).len() > MAX_SNIPPET_VARIABLES {
        return Err(VaultError::InvalidSnippet);
    }
    Ok(content)
}

fn check_snippet_description(description: &str) -> Result<(), VaultError> {
    let bad_char = description
        .chars()
        .any(|c| c.is_control() || is_invisible_format(c));
    if description.chars().count() > MAX_SNIPPET_DESCRIPTION_LEN || bad_char {
        Err(VaultError::InvalidSnippet)
    } else {
        Ok(())
    }
}

impl VaultData {
    /// Vérifie un contenu relu du disque : version de schéma et bornes.
    pub(crate) fn check_loaded(&self) -> Result<(), VaultError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(VaultError::UnsupportedVersion);
        }
        if self.hosts.len() > MAX_HOSTS
            || self.groups.len() > MAX_ENTRIES
            || self.identities.len() > MAX_ENTRIES
            || self.keys.len() > MAX_ENTRIES
            || self.known_hosts.len() > MAX_KNOWN_HOSTS_LEN
            || self.snippets.len() > MAX_SNIPPETS
        {
            return Err(VaultError::InvalidFile);
        }
        // Les snippets sont envoyés au terminal : leur contenu est revérifié à chaque chargement.
        for snippet in &self.snippets {
            let valid = clean_name(&snippet.name).is_ok_and(|n| n == snippet.name)
                && clean_snippet_content(&snippet.content).is_ok_and(|c| c == snippet.content)
                && check_snippet_description(&snippet.description).is_ok();
            if !valid {
                return Err(VaultError::InvalidFile);
            }
        }
        Ok(())
    }

    /// Hôtes.
    pub fn hosts(&self) -> &[Host] {
        &self.hosts
    }

    /// Groupes.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Identités.
    pub fn identities(&self) -> &[Identity] {
        &self.identities
    }

    /// Clés importées.
    pub fn keys(&self) -> &[StoredKey] {
        &self.keys
    }

    /// Réglages du coffre.
    pub fn settings(&self) -> &VaultSettings {
        &self.settings
    }

    /// Hôte par identifiant.
    pub fn host(&self, id: EntryId) -> Result<&Host, VaultError> {
        self.hosts
            .iter()
            .find(|h| h.id == id)
            .ok_or(VaultError::EntryNotFound)
    }

    /// Identité par identifiant.
    pub fn identity(&self, id: EntryId) -> Result<&Identity, VaultError> {
        self.identities
            .iter()
            .find(|i| i.id == id)
            .ok_or(VaultError::EntryNotFound)
    }

    /// Clé par identifiant.
    pub fn key(&self, id: EntryId) -> Result<&StoredKey, VaultError> {
        self.keys
            .iter()
            .find(|k| k.id == id)
            .ok_or(VaultError::EntryNotFound)
    }

    fn check_group(&self, id: Option<EntryId>) -> Result<(), VaultError> {
        match id {
            Some(id) if !self.groups.iter().any(|g| g.id == id) => Err(VaultError::EntryNotFound),
            _ => Ok(()),
        }
    }

    /// Crée ou modifie un hôte. Renvoie son identifiant.
    pub fn upsert_host(&mut self, input: HostInput) -> Result<EntryId, VaultError> {
        let name = clean_name(&input.name)?;
        let address = input.address.trim().to_owned();
        if address.is_empty()
            || address.len() > MAX_ADDRESS_LEN
            || address.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(VaultError::InvalidAddress);
        }
        if input.port == 0 {
            return Err(VaultError::InvalidPort);
        }
        // Sans identité, l'utilisateur est obligatoire.
        check_username(&input.username, input.identity_id.is_none())?;
        let tags = clean_tags(&input.tags)?;
        check_notes(&input.notes)?;
        if let Some(identity) = input.identity_id {
            self.identity(identity)?;
        }
        self.check_group(input.group_id)?;

        let host = |id| Host {
            id,
            name,
            address,
            port: input.port,
            username: input.username,
            identity_id: input.identity_id,
            group_id: input.group_id,
            tags,
            notes: input.notes,
        };
        match input.id {
            Some(id) => {
                let slot = self
                    .hosts
                    .iter_mut()
                    .find(|h| h.id == id)
                    .ok_or(VaultError::EntryNotFound)?;
                *slot = host(id);
                Ok(id)
            }
            None => {
                if self.hosts.len() >= MAX_HOSTS {
                    return Err(VaultError::TooManyEntries);
                }
                let id = Uuid::new_v4();
                self.hosts.push(host(id));
                Ok(id)
            }
        }
    }

    /// Supprime un hôte.
    pub fn delete_host(&mut self, id: EntryId) -> Result<(), VaultError> {
        let before = self.hosts.len();
        self.hosts.retain(|h| h.id != id);
        if self.hosts.len() == before {
            return Err(VaultError::EntryNotFound);
        }
        Ok(())
    }

    /// Crée ou renomme un groupe. Renvoie son identifiant.
    pub fn upsert_group(&mut self, id: Option<EntryId>, name: &str) -> Result<EntryId, VaultError> {
        let name = clean_name(name)?;
        match id {
            Some(id) => {
                let slot = self
                    .groups
                    .iter_mut()
                    .find(|g| g.id == id)
                    .ok_or(VaultError::EntryNotFound)?;
                slot.name = name;
                Ok(id)
            }
            None => {
                if self.groups.len() >= MAX_ENTRIES {
                    return Err(VaultError::TooManyEntries);
                }
                let id = Uuid::new_v4();
                self.groups.push(Group { id, name });
                Ok(id)
            }
        }
    }

    /// Supprime un groupe ; ses hôtes passent « sans groupe ».
    pub fn delete_group(&mut self, id: EntryId) -> Result<(), VaultError> {
        let before = self.groups.len();
        self.groups.retain(|g| g.id != id);
        if self.groups.len() == before {
            return Err(VaultError::EntryNotFound);
        }
        for host in &mut self.hosts {
            if host.group_id == Some(id) {
                host.group_id = None;
            }
        }
        Ok(())
    }

    /// Crée ou modifie une identité. Renvoie son identifiant.
    pub fn upsert_identity(&mut self, input: IdentityInput) -> Result<EntryId, VaultError> {
        let name = clean_name(&input.name)?;
        check_username(&input.username, true)?;
        if let PasswordChange::Set(secret) = &input.password {
            check_secret(secret)?;
        }
        if let Some(key) = input.key_id {
            self.key(key)?;
        }
        let existing = match input.id {
            Some(id) => Some(
                self.identities
                    .iter()
                    .position(|i| i.id == id)
                    .ok_or(VaultError::EntryNotFound)?,
            ),
            None => None,
        };
        let password = match input.password {
            PasswordChange::Keep => existing
                .and_then(|index| self.identities.get(index))
                .and_then(|i| i.password.clone()),
            PasswordChange::Clear => None,
            PasswordChange::Set(secret) => Some(secret),
        };
        if password.is_none() && input.key_id.is_none() {
            return Err(VaultError::IdentityIncomplete);
        }
        match existing.and_then(|index| self.identities.get_mut(index)) {
            Some(slot) => {
                slot.name = name;
                slot.username = input.username;
                slot.password = password;
                slot.key_id = input.key_id;
                Ok(slot.id)
            }
            None => {
                if self.identities.len() >= MAX_ENTRIES {
                    return Err(VaultError::TooManyEntries);
                }
                let id = Uuid::new_v4();
                self.identities.push(Identity {
                    id,
                    name,
                    username: input.username,
                    password,
                    key_id: input.key_id,
                });
                Ok(id)
            }
        }
    }

    /// Supprime une identité, sauf si un hôte l'utilise.
    pub fn delete_identity(&mut self, id: EntryId) -> Result<(), VaultError> {
        self.identity(id)?;
        if self.hosts.iter().any(|h| h.identity_id == Some(id)) {
            return Err(VaultError::IdentityInUse);
        }
        self.identities.retain(|i| i.id != id);
        Ok(())
    }

    /// Ajoute une clé importée. Renvoie son identifiant.
    pub fn add_key(&mut self, key: NewKey) -> Result<EntryId, VaultError> {
        let name = clean_name(&key.name)?;
        if self.keys.len() >= MAX_ENTRIES {
            return Err(VaultError::TooManyEntries);
        }
        let id = Uuid::new_v4();
        self.keys.push(StoredKey {
            id,
            name,
            algorithm: key.algorithm,
            fingerprint: key.fingerprint,
            public_key: key.public_key,
            private_key: key.private_key,
        });
        Ok(id)
    }

    /// Renomme une clé.
    pub fn rename_key(&mut self, id: EntryId, name: &str) -> Result<(), VaultError> {
        let name = clean_name(name)?;
        let slot = self
            .keys
            .iter_mut()
            .find(|k| k.id == id)
            .ok_or(VaultError::EntryNotFound)?;
        slot.name = name;
        Ok(())
    }

    /// Supprime une clé, sauf si une identité l'utilise.
    pub fn delete_key(&mut self, id: EntryId) -> Result<(), VaultError> {
        self.key(id)?;
        if self.identities.iter().any(|i| i.key_id == Some(id)) {
            return Err(VaultError::KeyInUse);
        }
        self.keys.retain(|k| k.id != id);
        Ok(())
    }

    /// Clés d'hôte connues, au format `known_hosts`.
    pub fn known_hosts(&self) -> &str {
        &self.known_hosts
    }

    /// Remplace les clés d'hôte connues.
    pub fn set_known_hosts(&mut self, content: String) -> Result<(), VaultError> {
        if content.len() > MAX_KNOWN_HOSTS_LEN {
            return Err(VaultError::KnownHostsTooLarge);
        }
        self.known_hosts = content;
        Ok(())
    }

    /// Snippets.
    pub fn snippets(&self) -> &[Snippet] {
        &self.snippets
    }

    /// Crée ou modifie un snippet. Renvoie son identifiant.
    pub fn upsert_snippet(&mut self, input: SnippetInput) -> Result<EntryId, VaultError> {
        let name = clean_name(&input.name)?;
        let content = clean_snippet_content(&input.content)?;
        let description = input.description.trim().to_owned();
        check_snippet_description(&description)?;
        let snippet = |id| Snippet {
            id,
            name,
            content,
            description,
        };
        match input.id {
            Some(id) => {
                let slot = self
                    .snippets
                    .iter_mut()
                    .find(|s| s.id == id)
                    .ok_or(VaultError::EntryNotFound)?;
                *slot = snippet(id);
                Ok(id)
            }
            None => {
                if self.snippets.len() >= MAX_SNIPPETS {
                    return Err(VaultError::TooManyEntries);
                }
                let id = Uuid::new_v4();
                self.snippets.push(snippet(id));
                Ok(id)
            }
        }
    }

    /// Supprime un snippet.
    pub fn delete_snippet(&mut self, id: EntryId) -> Result<(), VaultError> {
        let before = self.snippets.len();
        self.snippets.retain(|s| s.id != id);
        if self.snippets.len() == before {
            return Err(VaultError::EntryNotFound);
        }
        Ok(())
    }

    /// Change le délai de verrouillage automatique.
    pub fn set_auto_lock(&mut self, minutes: u32) -> Result<(), VaultError> {
        if !AUTO_LOCK_CHOICES.contains(&minutes) {
            return Err(VaultError::InvalidAutoLock);
        }
        self.settings.auto_lock_minutes = minutes;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_input(name: &str) -> HostInput {
        HostInput {
            id: None,
            name: name.into(),
            address: "srv.lab".into(),
            port: 22,
            username: "admin".into(),
            identity_id: None,
            group_id: None,
            tags: vec!["lab".into()],
            notes: String::new(),
        }
    }

    fn identity_input(password: Option<&str>, key_id: Option<EntryId>) -> IdentityInput {
        IdentityInput {
            id: None,
            name: "Admin".into(),
            username: "admin".into(),
            password: password.map_or(PasswordChange::Clear, |p| {
                PasswordChange::Set(SecretString::from(p))
            }),
            key_id,
        }
    }

    fn new_key() -> NewKey {
        NewKey {
            name: "Ma clé".into(),
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:abc".into(),
            public_key: "ssh-ed25519 AAAA".into(),
            private_key: SecretString::from("-----BEGIN OPENSSH PRIVATE KEY-----"),
        }
    }

    #[test]
    fn hosts_are_created_updated_and_deleted() {
        let mut data = VaultData::default();
        let id = data.upsert_host(host_input("  Web  ")).unwrap();
        assert_eq!(data.host(id).unwrap().name, "Web");
        let mut update = host_input("Web 2");
        update.id = Some(id);
        update.port = 2222;
        assert_eq!(data.upsert_host(update).unwrap(), id);
        assert_eq!(data.hosts().len(), 1);
        assert_eq!(data.host(id).unwrap().port, 2222);
        data.delete_host(id).unwrap();
        assert_eq!(data.delete_host(id), Err(VaultError::EntryNotFound));
    }

    #[test]
    fn host_fields_are_validated() {
        let mut data = VaultData::default();
        type Mutation = fn(&mut HostInput);
        let cases: [(Mutation, VaultError); 8] = [
            (|h| h.name = String::new(), VaultError::InvalidName),
            (|h| h.name = "a\u{1b}b".into(), VaultError::InvalidName),
            (
                |h| h.name = "x".repeat(MAX_NAME_LEN + 1),
                VaultError::InvalidName,
            ),
            (|h| h.address = "a b".into(), VaultError::InvalidAddress),
            (|h| h.port = 0, VaultError::InvalidPort),
            (|h| h.username = String::new(), VaultError::InvalidUsername),
            (
                |h| h.tags = vec!["t".into(); MAX_TAGS + 1],
                VaultError::InvalidTags,
            ),
            (|h| h.notes = "a\u{7}".into(), VaultError::InvalidNotes),
        ];
        for (mutate, expected) in cases {
            let mut input = host_input("ok");
            mutate(&mut input);
            assert_eq!(data.upsert_host(input), Err(expected));
        }
        let mut input = host_input("ok");
        input.notes = "ligne 1\n\tligne 2".into();
        assert!(data.upsert_host(input).is_ok());
    }

    #[test]
    fn duplicate_tags_are_merged() {
        let mut data = VaultData::default();
        let mut input = host_input("ok");
        input.tags = vec!["Prod".into(), "prod".into(), " web ".into()];
        let id = data.upsert_host(input).unwrap();
        assert_eq!(data.host(id).unwrap().tags, vec!["Prod", "web"]);
    }

    #[test]
    fn references_must_exist() {
        let mut data = VaultData::default();
        let mut input = host_input("ok");
        input.identity_id = Some(Uuid::new_v4());
        assert_eq!(data.upsert_host(input), Err(VaultError::EntryNotFound));
        let mut input = host_input("ok");
        input.group_id = Some(Uuid::new_v4());
        assert_eq!(data.upsert_host(input), Err(VaultError::EntryNotFound));
        assert_eq!(
            data.upsert_identity(identity_input(None, Some(Uuid::new_v4())))
                .err(),
            Some(VaultError::EntryNotFound)
        );
    }

    #[test]
    fn host_with_identity_may_omit_username() {
        let mut data = VaultData::default();
        let identity = data
            .upsert_identity(identity_input(Some("pw"), None))
            .unwrap();
        let mut input = host_input("ok");
        input.username = String::new();
        input.identity_id = Some(identity);
        assert!(data.upsert_host(input).is_ok());
    }

    #[test]
    fn deleting_a_group_ungroups_its_hosts() {
        let mut data = VaultData::default();
        let group = data.upsert_group(None, "Lab").unwrap();
        let mut input = host_input("ok");
        input.group_id = Some(group);
        let host = data.upsert_host(input).unwrap();
        data.upsert_group(Some(group), "Lab 2").unwrap();
        assert_eq!(data.groups()[0].name, "Lab 2");
        data.delete_group(group).unwrap();
        assert_eq!(data.host(host).unwrap().group_id, None);
    }

    #[test]
    fn identities_need_a_password_or_a_key() {
        let mut data = VaultData::default();
        assert_eq!(
            data.upsert_identity(identity_input(None, None)).err(),
            Some(VaultError::IdentityIncomplete)
        );
        assert_eq!(
            data.upsert_identity(identity_input(Some(""), None)).err(),
            Some(VaultError::InvalidSecret)
        );
        let key = data.add_key(new_key()).unwrap();
        let id = data
            .upsert_identity(identity_input(Some("pw"), Some(key)))
            .unwrap();

        // « Garder » conserve le mot de passe ; retirer la clé reste possible grâce à lui.
        let keep = IdentityInput {
            id: Some(id),
            name: "Admin".into(),
            username: "root".into(),
            password: PasswordChange::Keep,
            key_id: None,
        };
        data.upsert_identity(keep).unwrap();
        let identity = data.identity(id).unwrap();
        assert!(identity.password.is_some());
        assert_eq!(identity.username, "root");

        // Retirer le mot de passe sans clé : refusé.
        let clear = IdentityInput {
            id: Some(id),
            name: "Admin".into(),
            username: "root".into(),
            password: PasswordChange::Clear,
            key_id: None,
        };
        assert_eq!(
            data.upsert_identity(clear).err(),
            Some(VaultError::IdentityIncomplete)
        );
    }

    #[test]
    fn used_keys_and_identities_cannot_be_deleted() {
        let mut data = VaultData::default();
        let key = data.add_key(new_key()).unwrap();
        let identity = data
            .upsert_identity(identity_input(None, Some(key)))
            .unwrap();
        let mut input = host_input("ok");
        input.identity_id = Some(identity);
        let host = data.upsert_host(input).unwrap();

        assert_eq!(data.delete_key(key), Err(VaultError::KeyInUse));
        assert_eq!(
            data.delete_identity(identity),
            Err(VaultError::IdentityInUse)
        );
        data.delete_host(host).unwrap();
        data.delete_identity(identity).unwrap();
        data.rename_key(key, "Autre").unwrap();
        data.delete_key(key).unwrap();
        assert!(data.keys().is_empty());
    }

    #[test]
    fn auto_lock_accepts_only_the_proposed_values() {
        let mut data = VaultData::default();
        assert_eq!(data.settings().auto_lock_minutes, 15);
        data.set_auto_lock(5).unwrap();
        assert_eq!(data.set_auto_lock(0), Err(VaultError::InvalidAutoLock));
        assert_eq!(data.set_auto_lock(7), Err(VaultError::InvalidAutoLock));
        assert_eq!(data.settings().auto_lock_minutes, 5);
    }

    #[test]
    fn known_hosts_are_bounded() {
        let mut data = VaultData::default();
        data.set_known_hosts("srv ssh-ed25519 AAAA\n".into())
            .unwrap();
        assert_eq!(
            data.set_known_hosts("x".repeat(MAX_KNOWN_HOSTS_LEN + 1)),
            Err(VaultError::KnownHostsTooLarge)
        );
    }

    #[test]
    fn debug_never_shows_secrets() {
        let mut data = VaultData::default();
        let key = data.add_key(new_key()).unwrap();
        data.upsert_identity(identity_input(Some("hunter2"), Some(key)))
            .unwrap();
        let shown = format!("{data:?} {:?} {:?}", data.identities(), data.keys());
        assert!(!shown.contains("hunter2"));
        assert!(!shown.contains("PRIVATE KEY"));
    }

    #[test]
    fn serialization_round_trips_secrets_inside_the_vault() {
        let mut data = VaultData::default();
        let key = data.add_key(new_key()).unwrap();
        data.upsert_identity(identity_input(Some("hunter2"), Some(key)))
            .unwrap();
        let json = serde_json::to_string(&data).unwrap();
        let back: VaultData = serde_json::from_str(&json).unwrap();
        back.check_loaded().unwrap();
        assert_eq!(
            back.identities()[0]
                .password
                .as_ref()
                .unwrap()
                .expose_secret(),
            "hunter2"
        );
        // Un champ inconnu est refusé (fichier fabriqué ou version future).
        let tampered = json.replacen("\"hosts\"", "\"extra\":1,\"hosts\"", 1);
        assert!(serde_json::from_str::<VaultData>(&tampered).is_err());
    }

    fn snippet_input(content: &str) -> SnippetInput {
        SnippetInput {
            id: None,
            name: "Redémarrer".into(),
            content: content.into(),
            description: String::new(),
        }
    }

    #[test]
    fn snippets_are_created_updated_and_deleted() {
        let mut data = VaultData::default();
        let id = data
            .upsert_snippet(snippet_input("sudo systemctl restart {{service}}"))
            .unwrap();
        assert_eq!(data.snippets().len(), 1);
        let mut edit = snippet_input("ls -la\r\npwd");
        edit.id = Some(id);
        edit.description = "  liste  ".into();
        data.upsert_snippet(edit).unwrap();
        let snippet = &data.snippets()[0];
        assert_eq!(snippet.content, "ls -la\npwd");
        assert_eq!(snippet.description, "liste");
        let mut unknown = snippet_input("ls");
        unknown.id = Some(Uuid::new_v4());
        assert_eq!(data.upsert_snippet(unknown), Err(VaultError::EntryNotFound));
        data.delete_snippet(id).unwrap();
        assert_eq!(data.delete_snippet(id), Err(VaultError::EntryNotFound));
    }

    #[test]
    fn snippet_content_is_validated() {
        let mut data = VaultData::default();
        for bad in [
            "",
            "   \n ",
            "echo \u{1b}[31m",
            "a\rb",
            "echo \u{7f}",
            "echo \u{0}",
            "echo \u{9b}",
        ] {
            assert_eq!(
                data.upsert_snippet(snippet_input(bad)),
                Err(VaultError::InvalidSnippet),
                "{bad:?}"
            );
        }
        let too_long = "a".repeat(MAX_SNIPPET_LEN + 1);
        assert_eq!(
            data.upsert_snippet(snippet_input(&too_long)),
            Err(VaultError::InvalidSnippet)
        );
        let many: String = (0..=MAX_SNIPPET_VARIABLES)
            .map(|i| format!("{{{{v{i}}}}} "))
            .collect();
        assert_eq!(
            data.upsert_snippet(snippet_input(&many)),
            Err(VaultError::InvalidSnippet)
        );
        assert!(data.upsert_snippet(snippet_input("a\tb\nc")).is_ok());
        let mut bad_desc = snippet_input("ls");
        bad_desc.description = "a\nb".into();
        assert_eq!(
            data.upsert_snippet(bad_desc),
            Err(VaultError::InvalidSnippet)
        );
        for invisible in ["ls\u{202E}fdp", "ls\u{200B}", "\u{FEFF}ls", "a\u{2066}b"] {
            assert_eq!(
                data.upsert_snippet(snippet_input(invisible)),
                Err(VaultError::InvalidSnippet),
                "{invisible:?}"
            );
        }
        let mut bad_name = snippet_input("ls");
        bad_name.name = " ".into();
        assert_eq!(data.upsert_snippet(bad_name), Err(VaultError::InvalidName));
    }

    #[test]
    fn snippet_variables_are_extracted() {
        assert_eq!(
            snippet_variables("a {{service}} {{ x }} {{_n-1}} {{service}} {{9a}} {{{{b}}"),
            vec!["service", "_n-1", "b"]
        );
        assert_eq!(snippet_variables("{{a}"), Vec::<&str>::new());
        assert_eq!(snippet_variables("{{}} {{é}}"), Vec::<&str>::new());
        let long = format!("{{{{{}}}}}", "a".repeat(MAX_VARIABLE_NAME_LEN + 1));
        assert!(snippet_variables(&long).is_empty());
        assert!(is_variable_name("A_b-9"));
        assert!(!is_variable_name("-a"));
    }

    #[test]
    fn vault_without_snippets_still_loads() {
        let mut json = serde_json::to_value(VaultData::default()).unwrap();
        json.as_object_mut().unwrap().remove("snippets");
        let data: VaultData = serde_json::from_value(json).unwrap();
        assert!(data.snippets().is_empty());
        data.check_loaded().unwrap();
    }

    #[test]
    fn invalid_snippets_are_rejected_on_load() {
        let mut data = VaultData::default();
        let snippet = Snippet {
            id: Uuid::new_v4(),
            name: "x".into(),
            content: "ls".into(),
            description: String::new(),
        };
        data.snippets = vec![snippet.clone(); MAX_SNIPPETS + 1];
        assert_eq!(data.check_loaded(), Err(VaultError::InvalidFile));
        data.snippets = vec![snippet.clone()];
        data.check_loaded().unwrap();
        for bad in ["echo \u{1b}[31m", "", "a\r\nb", "ls\u{202E}"] {
            data.snippets = vec![Snippet {
                content: bad.into(),
                ..snippet.clone()
            }];
            assert_eq!(data.check_loaded(), Err(VaultError::InvalidFile), "{bad:?}");
        }
    }
}
