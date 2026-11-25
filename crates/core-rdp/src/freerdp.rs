//! Arguments et entrée standard de FreeRDP (`xfreerdp3` / `xfreerdp`).
//!
//! Le mot de passe ne figure **jamais** dans les arguments : il est lu sur l'entrée standard
//! (`/from-stdin:force`), que le cœur écrit puis ferme aussitôt.

use std::ffi::OsString;

use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

use crate::RdpError;
use crate::target::{RdpTarget, Resolution};

/// Longueur maximale d'un mot de passe transmis.
pub const MAX_PASSWORD_LEN: usize = 1024;

/// Arguments, un par option (aucun shell). `with_password` : lecture des identifiants sur
/// l'entrée standard avant la connexion.
pub fn freerdp_args(target: &RdpTarget, with_password: bool) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        format!("/v:{}", target.host_port()).into(),
        format!("/u:{}", target.username()).into(),
    ];
    if !target.domain().is_empty() {
        args.push(format!("/d:{}", target.domain()).into());
    }
    match target.resolution() {
        Resolution::Fullscreen => args.push("/f".into()),
        Resolution::Fixed { width, height } => args.push(format!("/size:{width}x{height}").into()),
    }
    // Premier certificat accepté et mémorisé, changement refusé **sans question** (`deny`) : le
    // refus ne dépend jamais de ce qui est lu sur l'entrée standard. Même règle que les clés
    // d'hôte SSH (TOFU). Jamais `/cert:ignore`.
    args.push("/cert:tofu,deny".into());
    args.push("+clipboard".into());
    if with_password {
        args.push("/from-stdin:force".into());
    }
    args
}

/// Contenu de l'entrée standard : FreeRDP 3 demande le domaine s'il n'en a aucun (ni `/d:`, ni
/// `DOMAINE\utilisateur` ; la forme `utilisateur@domaine` ne remplit pas le domaine), puis le
/// mot de passe, une ligne chacun. Si une version ne pose pas la question du domaine, la ligne
/// vide est lue comme mot de passe : l'authentification échoue, sans fuite (le reste du tube
/// n'est jamais lu).
pub fn freerdp_stdin(
    target: &RdpTarget,
    password: &SecretString,
) -> Result<Zeroizing<Vec<u8>>, RdpError> {
    let secret = password.expose_secret();
    if secret.is_empty() || secret.len() > MAX_PASSWORD_LEN || secret.contains(['\n', '\r']) {
        return Err(RdpError::InvalidPassword);
    }
    let mut input = Zeroizing::new(Vec::with_capacity(secret.len() + 2));
    if target.domain().is_empty() && !target.username().contains('\\') {
        input.push(b'\n');
    }
    input.extend_from_slice(secret.as_bytes());
    input.push(b'\n');
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(username: &str, domain: &str) -> RdpTarget {
        RdpTarget::new("win.lab", 3389, username, domain, Resolution::Fullscreen).unwrap()
    }

    #[test]
    fn arguments_never_carry_the_password() {
        let args = freerdp_args(&target("alice", "LAB"), true);
        let args: Vec<_> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "/v:win.lab:3389",
                "/u:alice",
                "/d:LAB",
                "/f",
                "/cert:tofu,deny",
                "+clipboard",
                "/from-stdin:force"
            ]
        );
        assert!(!args.iter().any(|a| a.starts_with("/p:")));
    }

    #[test]
    fn fixed_size_without_password() {
        let t = RdpTarget::new(
            "srv",
            3389,
            "a",
            "",
            Resolution::Fixed {
                width: 1280,
                height: 800,
            },
        )
        .unwrap();
        let args: Vec<_> = freerdp_args(&t, false)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"/size:1280x800".to_owned()));
        assert!(
            !args
                .iter()
                .any(|a| a.contains("stdin") || a.starts_with("/d:"))
        );
    }

    #[test]
    fn stdin_answers_the_domain_question_only_when_needed() {
        let pw = SecretString::from("s3cret pass");
        assert_eq!(
            &*freerdp_stdin(&target("alice", ""), &pw).unwrap(),
            b"\ns3cret pass\n"
        );
        assert_eq!(
            &*freerdp_stdin(&target("alice", "LAB"), &pw).unwrap(),
            b"s3cret pass\n"
        );
        assert_eq!(
            &*freerdp_stdin(&target("LAB\\alice", ""), &pw).unwrap(),
            b"s3cret pass\n"
        );
        assert_eq!(
            &*freerdp_stdin(&target("alice@lab", ""), &pw).unwrap(),
            b"\ns3cret pass\n"
        );
    }

    #[test]
    fn unusable_passwords_are_refused() {
        for bad in ["", "a\nb", "a\rb"] {
            assert_eq!(
                freerdp_stdin(&target("a", ""), &SecretString::from(bad)).unwrap_err(),
                RdpError::InvalidPassword
            );
        }
        let long = SecretString::from("a".repeat(MAX_PASSWORD_LEN + 1));
        assert_eq!(
            freerdp_stdin(&target("a", ""), &long).unwrap_err(),
            RdpError::InvalidPassword
        );
    }
}
