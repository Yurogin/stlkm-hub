use serde::{Deserialize, Serialize};

/// Les plateformes que le hub sait piloter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
    /// Le hub mobile. Ce programme-ci ne tourne jamais dessus : la plateforme existe
    /// pour que le catalogue puisse décrire les applis Android, et l'Atelier les montrer.
    Android,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }

    /// L'inverse de [`as_str`] : ce que l'écran renvoie quand on range une fiche.
    pub fn parse(nom: &str) -> Option<Self> {
        match nom.trim().to_ascii_lowercase().as_str() {
            "windows" => Some(Platform::Windows),
            "linux" => Some(Platform::Linux),
            "android" => Some(Platform::Android),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Android => "android",
        }
    }
}

/// Une valeur du catalogue qui peut être identique partout, ou dépendre de l'OS.
///
/// ```toml
/// exec = "outil.exe"                              # pareil partout
/// exec = { windows = "outil.exe", linux = "outil" } # au cas par cas
/// ```
///
/// Côté `android`, la valeur dit l'APK à prendre dans la release (`asset`) et le nom de
/// paquet de l'appli installée (`exec`) : c'est ainsi que le hub mobile la reconnaît.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlatformValue {
    Same(String),
    PerPlatform {
        #[serde(default)]
        windows: Option<String>,
        #[serde(default)]
        linux: Option<String>,
        #[serde(default)]
        android: Option<String>,
    },
}

impl PlatformValue {
    pub fn get(&self, platform: Platform) -> Option<&str> {
        match self {
            PlatformValue::Same(v) => Some(v.as_str()),
            PlatformValue::PerPlatform {
                windows,
                linux,
                android,
            } => match platform {
                Platform::Windows => windows.as_deref(),
                Platform::Linux => linux.as_deref(),
                Platform::Android => android.as_deref(),
            },
        }
    }

    /// Vrai si la valeur existe pour au moins une plateforme.
    pub fn is_empty(&self) -> bool {
        self.get(Platform::Windows).is_none()
            && self.get(Platform::Linux).is_none()
            && self.get(Platform::Android).is_none()
    }
}
