//! Fichier `.rdp` pour mstsc. Il ne contient **aucun mot de passe** : celui-ci passe par le
//! gestionnaire d'identifiants de Windows (SECURITE.md §4.3). Les valeurs viennent d'une
//! [`RdpTarget`] validée (aucun retour à la ligne possible).

use crate::target::{RdpTarget, Resolution};

/// Contenu du fichier `.rdp`. Redirections réduites au presse-papiers : ni disques, ni
/// imprimantes, ni cartes à puce, ni ports.
pub fn rdp_file(target: &RdpTarget) -> String {
    let mut lines = vec![
        format!("full address:s:{}", target.host_port()),
        format!("username:s:{}", target.username()),
    ];
    if !target.domain().is_empty() {
        lines.push(format!("domain:s:{}", target.domain()));
    }
    match target.resolution() {
        Resolution::Fullscreen => lines.push("screen mode id:i:2".into()),
        Resolution::Fixed { width, height } => {
            lines.push("screen mode id:i:1".into());
            lines.push(format!("desktopwidth:i:{width}"));
            lines.push(format!("desktopheight:i:{height}"));
        }
    }
    lines.extend(
        [
            // NLA (CredSSP) ; mstsc avertit si le certificat du serveur n'est pas vérifiable.
            "enablecredsspsupport:i:1",
            "authentication level:i:2",
            "prompt for credentials:i:0",
            "redirectclipboard:i:1",
            "redirectprinters:i:0",
            "redirectcomports:i:0",
            "redirectsmartcards:i:0",
            "drivestoredirect:s:",
            "devicestoredirect:s:",
        ]
        .map(String::from),
    );
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_only_fixed_keys_and_no_password() {
        let target = RdpTarget::new(
            "win.lab",
            3390,
            "alice",
            "LAB",
            Resolution::Fixed {
                width: 1920,
                height: 1080,
            },
        )
        .unwrap();
        let text = rdp_file(&target);
        assert!(
            text.starts_with("full address:s:win.lab:3390\r\nusername:s:alice\r\ndomain:s:LAB\r\n")
        );
        assert!(text.contains("desktopwidth:i:1920\r\ndesktopheight:i:1080"));
        assert!(text.contains("drivestoredirect:s:\r\n"));
        assert!(!text.to_lowercase().contains("password"));
        // Une ligne par clé : chaque ligne commence par une clé connue.
        for line in text.lines() {
            assert!(line.contains(":s:") || line.contains(":i:"), "{line}");
        }
    }

    #[test]
    fn fullscreen_without_domain() {
        let target = RdpTarget::new("srv", 3389, "a", "", Resolution::Fullscreen).unwrap();
        let text = rdp_file(&target);
        assert!(text.contains("screen mode id:i:2"));
        assert!(!text.contains("domain:s:"));
    }
}
