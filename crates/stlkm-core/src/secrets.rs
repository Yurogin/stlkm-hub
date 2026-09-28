//! Un garde-fou avant publication : rien de ce qui ressemble à une clé ne doit
//! partir sur GitHub. Volontairement bête et sans regex — il vaut mieux une
//! fausse alerte qu'un token publié.

/// Ce qui a été repéré, et où.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub reason: String,
}

/// Préfixes qui ne laissent aucun doute.
const KNOWN_PREFIXES: &[(&str, &str)] = &[
    ("ghp_", "token GitHub personnel"),
    ("gho_", "token GitHub OAuth"),
    ("ghs_", "token GitHub serveur"),
    ("github_pat_", "token GitHub"),
    ("xoxb-", "token Slack"),
    ("xoxp-", "token Slack"),
    ("sk-ant-", "clé API Anthropic"),
    ("sk-proj-", "clé API OpenAI"),
    ("AKIA", "clé AWS"),
    ("AIza", "clé Google"),
    ("-----BEGIN ", "clé privée"),
];

/// Mots qui, suivis d'une valeur assez longue, sentent le secret.
const SUSPICIOUS_KEYS: &[&str] = &[
    "token", "password", "passwd", "secret", "api_key", "apikey", "private_key",
];

/// Inspecte un texte et retourne ce qui ne devrait pas être publié.
pub fn sniff(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;

        // Un commentaire n'est pas inoffensif : on y colle souvent une clé.
        for (prefix, what) in KNOWN_PREFIXES {
            if line.contains(prefix) {
                findings.push(Finding {
                    line: number,
                    reason: format!("{what} ({prefix}…)"),
                });
            }
        }

        let lowered = line.to_ascii_lowercase();
        for key in SUSPICIOUS_KEYS {
            let Some(at) = lowered.find(key) else {
                continue;
            };
            // On ne s'inquiète que si une valeur suit vraiment le mot-clé.
            let after = &line[at + key.len()..];
            let Some(value) = after.split_once(['=', ':']).map(|(_, v)| v.trim()) else {
                continue;
            };
            let value = value.trim_matches(['"', '\'', ',']);
            if value.len() >= 12 && !value.contains(' ') {
                findings.push(Finding {
                    line: number,
                    reason: format!("« {key} » suivi d'une valeur de {} caractères", value.len()),
                });
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repere_un_token_github() {
        let found = sniff("repo = \"moi/truc\"\ntoken = ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345\n");
        assert!(!found.is_empty());
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn repere_un_mot_de_passe_en_dur() {
        let found = sniff("password = \"correct-horse-battery\"");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn laisse_passer_un_catalogue_normal() {
        let catalog = r#"
schema = 1

[[project]]
id = "portfolio"
name = "Portfolio"
source = { kind = "github-repo", repo = "Gabriel-SEMPERE/Portfolio" }
launch = { kind = "static" }
"#;
        assert!(sniff(catalog).is_empty());
    }

    #[test]
    fn ne_salarme_pas_sur_une_phrase() {
        // Un résumé qui parle de mot de passe n'est pas un mot de passe.
        assert!(sniff("summary = \"Gestionnaire de password pour le bureau\"").is_empty());
    }
}
