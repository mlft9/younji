//! Import d'une configuration réaliste (fixture `tests/fixtures/test_ssh`), avec `Include` et
//! jokers (critère d'acceptation de l'étape 9).

use std::path::Path;

use core_import::{HostWarning, SkipReason, parse_ssh_config};

#[test]
fn realistic_config_with_include_and_wildcards() {
    let ssh = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_ssh");
    let report = parse_ssh_config(&ssh.join("config"), &ssh, &ssh);
    let summary: Vec<_> = report
        .hosts
        .iter()
        .map(|h| {
            (
                h.alias.as_str(),
                h.hostname.as_str(),
                h.port,
                h.user.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("pve.lab", "pve.lab", 8022, Some("labadmin")),
            ("nas.lab", "192.0.2.20", 22, Some("labadmin")),
            ("perso", "perso.example.test", 22, Some("defaut")),
            ("bastion", "bastion.example.test", 2222, Some("admin")),
            ("web-prod", "203.0.113.10", 22, Some("defaut")),
        ]
    );
    let web = &report.hosts[4];
    assert!(web.warnings.contains(&HostWarning::ProxyIgnored));
    assert!(
        report.hosts[0]
            .warnings
            .contains(&HostWarning::IdentityFileNotImported)
    );
    assert!(
        report.hosts[2]
            .warnings
            .contains(&HostWarning::ForwardingIgnored)
    );
    let skipped: Vec<_> = report
        .skipped
        .iter()
        .map(|s| (s.entry.as_str(), s.reason))
        .collect();
    assert!(skipped.contains(&("Host *.lab", SkipReason::Wildcard)));
    assert!(skipped.contains(&("Host *", SkipReason::Wildcard)));
    assert!(skipped.contains(&("Match host *.corp", SkipReason::MatchBlock)));
}
