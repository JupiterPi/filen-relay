use dioxus::fullstack::serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Server {
    pub id: i32,
    pub name: String,
    pub r#type: String,
    pub filen_email: String,
    pub filen_password: String,
}

#[cfg(feature = "server")]
pub(crate) mod conn {
    use dioxus::prelude::*;

    thread_local! {
        pub static DB: rusqlite::Connection = {
            let conn = rusqlite::Connection::open("file-relay.db").expect("Failed to open database");
            conn.execute_batch("
            CREATE TABLE IF NOT EXISTS allowed_users (
                id INTEGER PRIMARY KEY,
                email TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                type TEXT NOT NULL,
                filen_email TEXT NOT NULL,
                filen_password TEXT NOT NULL
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

    pub(crate) fn get_servers() -> Result<Vec<super::Server>> {
        DB.with(|db| {
            let mut stmt =
                db.prepare("SELECT id, name, type, filen_email, filen_password FROM servers")?;
            let server_iter = stmt.query_map([], |row| {
                Ok(super::Server {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    r#type: row.get(2)?,
                    filen_email: row.get(3)?,
                    filen_password: row.get(4)?,
                })
            })?;

            let mut servers = Vec::new();
            for server in server_iter {
                servers.push(server?);
            }
            Ok(servers)
        })
    }

    pub(crate) fn get_servers_for_user(email: &str) -> Result<Vec<super::Server>> {
        DB.with(|db| {
            let mut stmt = db.prepare(
                "SELECT id, name, type, filen_email, filen_password FROM servers WHERE filen_email = ?1",
            )?;
            let server_iter = stmt.query_map(rusqlite::params![email], |row| {
                Ok(super::Server {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    r#type: row.get(2)?,
                    filen_email: row.get(3)?,
                    filen_password: row.get(4)?,
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
        r#type: &str,
        filen_email: &str,
        filen_password: &str,
    ) -> Result<()> {
        DB.with(|db| {
            db.execute(
                "INSERT INTO servers (name, type, filen_email, filen_password) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![name, r#type, filen_email, filen_password],
            )?;
            Ok(())
        })
    }

    pub(crate) fn delete_server(id: i32) -> Result<()> {
        DB.with(|db| {
            db.execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])?;
            Ok(())
        })
    }
}
