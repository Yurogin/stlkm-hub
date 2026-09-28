//! Cœur du hub STLKM : ce crate ne connaît ni terminal ni fenêtre.
//! La CLI et la GUI Tauri s'en servent toutes les deux.

pub mod catalog;
pub mod catalog_edit;
pub mod config;
pub mod discover;
pub mod error;
pub mod github;
pub mod guess;
pub mod install;
pub mod mobile;
pub mod paths;
pub mod platform;
pub mod process;
pub mod publish;
pub mod repository;
pub mod run;
pub mod secrets;
pub mod server;
pub mod state;
pub mod update;

pub use catalog::{Catalog, Launch, Project, Source, SCHEMA_VERSION};
pub use config::{Config, Mode};
pub use error::{Error, Result};
pub use install::{install, uninstall};
pub use platform::{Platform, PlatformValue};
pub use run::{run, Session};
pub use publish::{plan, Plan};
pub use repository::Origin;
pub use state::{Installed, State};
pub use update::{check, check_all, Check, Status};
