use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ServerSpec {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    pub root: String,
    pub read_only: bool,
    pub password: Option<String>,
    pub filen_email: String,
    pub filen_password: String,
    pub filen_2fa_code: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, EnumIter)]
pub(crate) enum ServerType {
    Http,
    Webdav,
    S3,
    Ftp,
    Sftp,
}

impl Display for ServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerType::Http => write!(f, "HTTP"),
            ServerType::Webdav => write!(f, "WebDAV"),
            ServerType::S3 => write!(f, "S3"),
            ServerType::Ftp => write!(f, "FTP"),
            ServerType::Sftp => write!(f, "SFTP"),
        }
    }
}

impl From<&str> for ServerType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "http" => ServerType::Http,
            "webdav" => ServerType::Webdav,
            "s3" => ServerType::S3,
            "ftp" => ServerType::Ftp,
            "sftp" => ServerType::Sftp,
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
    Running { port: u16 },
    Error,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LogLine {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content: LogLineContent,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum LogLineContent {
    Event(String),
    ServerProcess(String),
}
