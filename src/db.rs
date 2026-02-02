use dioxus::prelude::*;

use crate::common::{ServerSpec, ServerType};

thread_local! {
    pub static DB: rusqlite::Connection = {
        let conn = rusqlite::Connection::open("file-relay.db").expect("Failed to open database");
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS allowed_users (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                server_type TEXT NOT NULL,
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
            db.prepare("SELECT id, name, server_type, filen_email, filen_password, filen_2fa_code FROM servers")?;
        let server_iter = stmt.query_map([], |row| {
            Ok(ServerSpec {
                id: row.get(0)?,
                name: row.get(1)?,
                server_type: row.get::<_, String>(2)?.as_str().into(),
                filen_email: row.get(3)?,
                filen_password: row.get(4)?,
                filen_2fa_code: row.get(5)?,
            })
        })?;

        let mut servers = Vec::new();
        for server in server_iter {
            servers.push(server?);
        }
        Ok(servers)
    })
}

pub(crate) fn create_server(
    name: &str,
    server_type: ServerType,
    filen_email: &str,
    filen_password: &str,
    filen_2fa_code: Option<&str>,
) -> Result<ServerSpec> {
    DB.with(|db| {
        let id = uuid::Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO servers (id, name, server_type, filen_email, filen_password, filen_2fa_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, name, server_type.to_string(), filen_email, filen_password, filen_2fa_code],
        )?;
        Ok(ServerSpec {
            id,
            name: name.to_string(),
            server_type,
            filen_email: filen_email.to_string(),
            filen_password: filen_password.to_string(),
            filen_2fa_code: filen_2fa_code.map(|code| code.to_string()),
        })
    })
}

pub(crate) fn delete_server(id: &str) -> Result<()> {
    DB.with(|db| {
        db.execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
}
