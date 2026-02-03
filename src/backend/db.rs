use std::sync::OnceLock;

use dioxus::prelude::*;

use crate::{
    backend,
    common::{ServerId, ServerSpec},
};

pub static DB_DIR: OnceLock<Option<String>> = OnceLock::new();

thread_local! {
    pub static DB: rusqlite::Connection = {
        let db_dir = backend::DB_DIR.get().unwrap().clone().unwrap_or(".".to_string()).trim_end_matches('/').to_string();
        let conn = rusqlite::Connection::open(format!("{}/file-relay.db", db_dir)).expect("Failed to open database");
        conn.execute_batch("
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
        ).unwrap();
        conn
    };
}

pub(crate) fn get_allowed_users() -> Result<Vec<String>> {
    DB.with(|db| {
        let mut stmt = db.prepare("SELECT email FROM allowed_users")?;
        let user_iter = stmt.query_map([], |row| row.get(0))?;

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    })
}

pub(crate) fn add_allowed_user(email: &str) -> Result<()> {
    DB.with(|db| {
        db.execute(
            "INSERT INTO allowed_users (email) VALUES (?1)",
            rusqlite::params![email],
        )?;
        Ok(())
    })
}

pub(crate) fn remove_allowed_user(email: &str) -> Result<()> {
    DB.with(|db| {
        db.execute(
            "DELETE FROM allowed_users WHERE email = ?1",
            rusqlite::params![email],
        )?;
        Ok(())
    })
}

pub(crate) fn clear_allowed_users() -> Result<()> {
    DB.with(|db| {
        db.execute("DELETE FROM allowed_users", [])?;
        Ok(())
    })
}

pub(crate) fn get_servers() -> Result<Vec<ServerSpec>> {
    DB.with(|db| {
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
    })
}

pub(crate) fn create_server(spec: &ServerSpec) -> Result<()> {
    DB.with(|db| {
        db.execute(
            "INSERT INTO servers (id, name, server_type, root, read_only, password, filen_email, filen_password, filen_2fa_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![spec.id, spec.name, spec.server_type.to_string(), spec.root, spec.read_only, spec.password, spec.filen_email, spec.filen_password, spec.filen_2fa_code],
        )?;
        Ok(())
    })
}

pub(crate) fn delete_server(id: &ServerId) -> Result<()> {
    DB.with(|db| {
        db.execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
}
