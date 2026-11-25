//! Nettoyage des textes affichés (venus d'un fichier de l'utilisateur, donc non fiables).

/// Longueur maximale d'un texte affiché.
pub const MAX_DISPLAY: usize = 256;

/// Remplace les caractères de contrôle et invisibles par `�` et tronque à [`MAX_DISPLAY`]
/// caractères.
pub fn display(text: &str) -> String {
    let mut out: String = text
        .chars()
        .take(MAX_DISPLAY)
        .map(|c| if is_unsafe(c) { '\u{FFFD}' } else { c })
        .collect();
    if text.chars().count() > MAX_DISPLAY {
        out.push('…');
    }
    out
}

/// Caractère de contrôle ou de mise en forme invisible (bidirectionnel, largeur nulle, BOM).
pub fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{FEFF}'
        )
}

/// Correspondance d'un motif OpenSSH (`*`, `?`), sans tenir compte de la casse ASCII.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().map(|c| c.to_ascii_lowercase()).collect();
    let t: Vec<char> = text.chars().map(|c| c.to_ascii_lowercase()).collect();
    // Algorithme glouton avec retour au dernier `*` : linéaire en pratique, sans récursion.
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some('*') => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some(&c) if c == '?' || Some(&c) == t.get(ti) => {
                pi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        assert!(wildcard_match("*.lab", "web.LAB"));
        assert!(wildcard_match("web?", "web1"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("a*b*c", "axxbyyc"));
        assert!(!wildcard_match("a*b*c", "axxbyy"));
        assert!(!wildcard_match("web?", "web"));
        assert!(!wildcard_match("*.lab", "lab"));
    }

    #[test]
    fn display_is_cleaned_and_bounded() {
        assert_eq!(display("a\u{1b}[31mb\u{202E}"), "a\u{FFFD}[31mb\u{FFFD}");
        assert_eq!(display(&"x".repeat(300)).chars().count(), MAX_DISPLAY + 1);
    }
}
