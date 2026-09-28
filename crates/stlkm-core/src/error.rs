#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("lecture impossible : {0}")]
    Io(#[from] std::io::Error),

    #[error("catalogue mal formé : {0}")]
    Toml(#[from] toml::de::Error),

    #[error("catalogue en schéma v{found}, ce hub comprend au plus v{max} — mets le hub à jour")]
    Schema { found: u32, max: u32 },

    #[error("deux projets partagent l'identifiant « {0} » dans le catalogue")]
    DuplicateId(String),

    #[error("projet inconnu : « {0} »")]
    UnknownProject(String),

    #[error("« {project} » n'a rien de prévu pour {platform}")]
    UnsupportedPlatform {
        project: String,
        platform: &'static str,
    },

    #[error("impossible de localiser les dossiers de l'utilisateur")]
    NoHome,

    #[error("état du hub illisible : {0}")]
    Json(#[from] serde_json::Error),

    #[error("échec réseau : {0}")]
    Http(String),

    #[error("archive illisible : {0}")]
    Archive(String),

    #[error("« {cmd} » a échoué ({code})")]
    CommandFailed { cmd: String, code: String },

    #[error("aucun fichier ne correspond à « {pattern} » dans la release {tag} de {repo}")]
    NoAsset {
        repo: String,
        tag: String,
        pattern: String,
    },

    #[error("{0} n'a aucune release publiée")]
    NoRelease(String),

    #[error("« {0} » n'est pas installé")]
    NotInstalled(String),

    #[error("le dossier local de « {0} » a disparu")]
    MissingLocalPath(String),

    #[error("catalogue introuvable : {0}")]
    MissingCatalog(String),

    #[error("{0} n'est dans aucun dépôt git : impossible de publier")]
    NoCatalogRepo(String),

    #[error("publication refusée : {0} secret(s) repéré(s) dans le catalogue")]
    SecretFound(usize),

    #[error("aucun projet « {0} » trouvé sur le disque")]
    UnknownCandidate(String),

    #[error("aucun répertoire configuré : indique le dépôt du catalogue")]
    NoCatalogSource,

    #[error("réservé au propriétaire du répertoire")]
    NotOwner,

    #[error("le répertoire n'a pas encore été publié ({0})")]
    NotPublished(String),

    #[error("« {0} » n'a rien installé : le dossier est vide")]
    InstallEmpty(String),

    #[error("le dépôt {0} est vide : son code n'a jamais été poussé sur GitHub")]
    EmptyRepo(String),

    #[error("impossible de deviner comment lancer « {0} » : précise-le dans le répertoire")]
    NoLaunch(String),

    #[error("« {0} » tourne encore : ferme-le puis réessaie")]
    StillRunning(String),
}

pub type Result<T> = std::result::Result<T, Error>;
