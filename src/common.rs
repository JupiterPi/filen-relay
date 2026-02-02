use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ServerSpec {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    pub filen_email: String,
    pub filen_password: String,
    pub filen_2fa_code: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, EnumIter)]
pub(crate) enum ServerType {
    Http,
    Webdav,
}

impl Display for ServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerType::Http => write!(f, "HTTP"),
            ServerType::Webdav => write!(f, "WebDAV"),
        }
    }
}

impl From<&str> for ServerType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "http" => ServerType::Http,
            "webdav" => ServerType::Webdav,
            _ => ServerType::Http,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ServerState {
    pub spec: ServerSpec,
    pub logs_id: String,
    pub status: ServerStatus,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum ServerStatus {
    Starting,
    Running { connection_url: String },
    Error,
}

// todo: tmp / move this to frontend
impl Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Starting => write!(f, "Offline"),
            ServerStatus::Running { connection_url, .. } => {
                write!(f, "Running (URL: {})", connection_url)
            }
            ServerStatus::Error => write!(f, "Error"),
        }
    }
}
