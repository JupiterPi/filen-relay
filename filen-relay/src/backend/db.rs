use std::sync::Mutex;

use dioxus::prelude::*;
use filen_sdk_rs::{
    auth::Client,
    fs::{file::enums::RemoteFileType, FSObject, HasUUID},
};
use filen_types::fs::UuidStr;
use rusqlite::Connection;

use crate::{
    common::{ServerId, ServerSpec},
    util::UnwrapOnceLock,
};

// todo: is it good (or safe) that this needs to be .lock().unwrap() everywhere?
pub(crate) static DB: UnwrapOnceLock<DbViaOfflineOrRemoteFile> = UnwrapOnceLock::new();

const DB_FILE_NAME: &str = "filen-relay.db";
pub(crate) struct DbViaOfflineOrRemoteFile {
    conn: Mutex<rusqlite::Connection>,
    filen_client: Option<Client>,
    remote_db_dir: Option<UuidStr>,
}

impl DbViaOfflineOrRemoteFile {
    pub(crate) async fn new_from_email_and_password(
        filen_email: String,
        filen_password: &str,
        filen_two_factor_code: Option<&str>,
    ) -> Result<Self> {
        let client = filen_sdk_rs::auth::Client::login(
            filen_email,
            filen_password,
            filen_two_factor_code.unwrap_or("XXXXXX"),
        )
        .await
        .context("Failed to log in to admin Filen")?;
        let remote_db_dir = Self::initialize_from_filen(&client).await?;
        let db = Self {
            conn: Mutex::new(Self::init(None)),
            filen_client: Some(client),
            remote_db_dir: Some(remote_db_dir),
        };
        Ok(db)
    }

    pub(crate) async fn new_from_auth_config(filen_auth_config: String) -> Result<(String, Self)> {
        let client = filen_cli::deserialize_auth_config(&filen_auth_config)
            .context("Failed to deserialize admin Filen auth config")?;
        let admin_email = client.email().to_string();
        let remote_db_dir = Self::initialize_from_filen(&client).await?;
        let db = Self {
            conn: Mutex::new(Self::init(None)),
            filen_client: Some(client),
            remote_db_dir: Some(remote_db_dir),
        };
        Ok((admin_email, db))
    }

    pub(crate) async fn new_from_offline_location(db_dir: Option<&str>) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(Self::init(db_dir)),
            filen_client: None,
            remote_db_dir: None,
        })
    }

    fn init(db_dir: Option<&str>) -> Connection {
        let db_dir = db_dir.unwrap_or(".").trim_end_matches('/').to_string();
        let conn = rusqlite::Connection::open(format!("{}/{}", db_dir, DB_FILE_NAME))
            .expect("Failed to open database");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS allowed_users (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                server_type TEXT NOT NULL,
                root TEXT NOT NULL,
                read_only BOOLEAN NOT NULL,
                password TEXT,
                filen_email TEXT NOT NULL,
                filen_password TEXT NOT NULL,
                filen_2fa_code TEXT
            );
            ",
        )
        .unwrap();
    conn
    }

    async fn initialize_from_filen(client: &Client) -> anyhow::Result<UuidStr> {
        let local_db_file = std::env::current_dir()?.join(DB_FILE_NAME);
        if tokio::fs::try_exists(&local_db_file).await.context("Failed to check if local database file exists")? {
            tokio::fs::remove_file(&local_db_file).await.context("Failed to remove existing local database file")?;
        }
        match client
            .find_item_at_path(&format!("/.filen-relay/{}", DB_FILE_NAME))
            .await?
        {
            Some(FSObject::File(file)) => {
                let db_file = RemoteFileType::File(file);
                client
                    .download_file_to_path(
                        &db_file,
                        local_db_file,
                        None,
                    )
                    .await?;
            }
            _ => {
                dioxus::logger::tracing::warn!(
                    "Filen relay database not found at /.filen-relay/{} in admin Filen account, starting with empty database",
                    DB_FILE_NAME
                );
            }
        };
        Ok(*client
            .find_or_create_dir(".filen-relay")
            .await
            .context("Failed to create .filen-relay dir in admin Filen account")?
            .uuid())
    }

    // todo: make this more async so that other things can be resumed until the upload is done (can be done at call site probably)
    async fn write_to_filen(&self) -> anyhow::Result<()> {
        let Some(client) = &self.filen_client else {
            return Ok(()); // it is not needed
        };
        client
            .upload_file_from_path(
                self.remote_db_dir.as_ref().unwrap(),
                std::env::current_dir()?.join(DB_FILE_NAME),
                None,
            )
            .await
            .context("Failed to upload database file to admin Filen account")?;
        Ok(())
    }

    pub(crate) fn get_allowed_users(&self) -> Result<Vec<String>> {
        let db = self.conn.lock().unwrap();
        let mut stmt = db.prepare("SELECT email FROM allowed_users")?;
        let user_iter = stmt.query_map([], |row| row.get(0))?;
        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    }

    pub(crate) async fn add_allowed_user(&self, email: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO allowed_users (email) VALUES (?1)",
            rusqlite::params![email],
        )?;
        self.write_to_filen().await?;
        Ok(())
    }

    pub(crate) async fn remove_allowed_user(&self, email: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM allowed_users WHERE email = ?1",
            rusqlite::params![email],
        )?;
        self.write_to_filen().await?;
        Ok(())
    }

    pub(crate) async fn clear_allowed_users(&self) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM allowed_users", [])?;
        self.write_to_filen().await?;
        Ok(())
    }

    pub(crate) fn get_servers(&self) -> Result<Vec<ServerSpec>> {
        let db = self.conn.lock().unwrap();
        let mut stmt = 
            db.prepare("SELECT id, name, server_type, root, read_only, password, filen_email, filen_password, filen_2fa_code FROM servers")?;
        let server_iter = stmt.query_map([], |row| {
            Ok(ServerSpec {
                id: row.get(0)?,
                name: row.get(1)?,
                server_type: row.get::<_, String>(2)?.as_str().into(),
                root: row.get(3)?,
                read_only: row.get(4)?,
                password: row.get(5)?,
                filen_email: row.get(6)?,
                filen_password: row.get(7)?,
                filen_2fa_code: row.get(8)?,
            })
        })?;
        let mut servers = Vec::new();
        for server in server_iter {
            servers.push(server?);
        }
        Ok(servers)
    }

    pub(crate) async fn create_server(&self, spec: &ServerSpec) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO servers (id, name, server_type, root, read_only, password, filen_email, filen_password, filen_2fa_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![spec.id, spec.name, spec.server_type.to_string(), spec.root, spec.read_only, spec.password, spec.filen_email, spec.filen_password, spec.filen_2fa_code],
        )?;
        self.write_to_filen().await?;
        Ok(())
    }

    pub(crate) async fn delete_server(&self, id: &ServerId) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])?;
        self.write_to_filen().await?;
        Ok(())
    }
}
