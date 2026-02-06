use crate::common::{LogLine, ServerId, ServerState, ServerType};
use dioxus::fullstack::{response::Response, JsonEncoding, Streaming};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::backend::{auth, db::DB, server_manager, server_manager::SERVER_MANAGER};

#[derive(Serialize, Deserialize)]
pub(crate) struct User {
    pub email: String,
    pub is_admin: bool,
}

#[post("/api/user", session: auth::Session)]
pub(crate) async fn get_user() -> Result<User> {
    Ok(User {
        email: session.filen_email,
        is_admin: session.is_admin,
    })
}

#[post("/api/login")]
pub(crate) async fn login(
    email: String,
    password: String,
    two_factor_code: Option<String>,
) -> Result<Response, anyhow::Error> {
    let token = auth::login_and_get_session_token(email, password, two_factor_code).await?;
    use dioxus::fullstack::{body::Body, response::Response};
    Ok(Response::builder()
        .header("Set-Cookie", format!("Session={}; HttpOnly; Path=/", token))
        .body(Body::empty())
        .unwrap())
}

#[post("/api/logout")]
pub(crate) async fn logout() -> Result<Response> {
    use dioxus::fullstack::{body::Body, response::Response};
    Ok(Response::builder()
        .header("Set-Cookie", "Session=; HttpOnly; Path=/")
        .body(Body::empty())
        .unwrap())
}

#[get("/api/servers", session: auth::Session)]
pub(crate) async fn get_servers() -> Result<Streaming<Vec<ServerState>, JsonEncoding>> {
    Ok(Streaming::spawn(move |tx| async move {
        let send_server_states = || {
            let server_states = SERVER_MANAGER
                .get_server_states()
                .borrow()
                .iter()
                .filter(|s| session.is_admin || s.spec.filen_email == session.filen_email)
                .cloned()
                .collect::<Vec<ServerState>>();
            if let Err(e) = tx.unbounded_send(server_states) {
                dioxus::logger::tracing::error!("Failed to send server states: {}", e);
                false
            } else {
                true
            }
        };
        let _ = send_server_states();
        let mut server_states = SERVER_MANAGER.get_server_states();
        loop {
            match server_states.changed().await {
                Err(e) => {
                    dioxus::logger::tracing::error!("Failed to watch server states: {}", e);
                    break;
                }
                Ok(()) => {
                    if !send_server_states() {
                        break;
                    }
                }
            }
        }
    }))
}

#[get("/api/logs/{logs_id}", session: auth::Session)]
pub(crate) async fn get_logs(logs_id: String) -> Result<Streaming<LogLine, JsonEncoding>> {
    let Some(logs) = SERVER_MANAGER.get_logs(&logs_id) else {
        return Err(anyhow::anyhow!("Logs not found"))?;
    };
    if !session.is_admin && logs.server_spec.filen_email != session.filen_email {
        return Err(anyhow::anyhow!("Unauthorized to access logs"))?;
    }
    Ok(Streaming::spawn(|tx| async move {
        let (history, mut rx) = {
            let logs = logs.logs.lock().unwrap();
            let (history, rx) = logs.get();
            (history.clone(), rx.resubscribe())
        };
        for line in history {
            if tx.unbounded_send(line.clone()).is_err() {
                return;
            }
        }
        while let Ok(line) = rx.recv().await {
            if tx.unbounded_send(line).is_err() {
                return;
            }
        }
    }))
}

#[post("/api/servers/add", session: auth::Session)]
pub(crate) async fn add_server(
    name: String,
    server_type: ServerType,
    root: String,
    read_only: bool,
    password: Option<String>,
) -> Result<(), anyhow::Error> {
    SERVER_MANAGER
        .update_server_spec(server_manager::ServerSpecUpdate::Add(
            crate::common::ServerSpec {
                id: ServerId::new(),
                name,
                server_type,
                root,
                read_only,
                password,
                filen_email: session.filen_email,
                filen_password: session.filen_password,
                filen_2fa_code: session.filen_2fa_code,
            },
        ))
        .await
}

#[post("/api/servers/remove", session: auth::Session)]
pub(crate) async fn remove_server(id: ServerId) -> Result<(), anyhow::Error> {
    SERVER_MANAGER
        .get_server_states()
        .borrow()
        .iter()
        .find(|s| {
            s.spec.id == id && (session.is_admin || s.spec.filen_email == session.filen_email)
        })
        .ok_or_else(|| anyhow::anyhow!("Server not found or not owned by user"))?;
    SERVER_MANAGER
        .update_server_spec(server_manager::ServerSpecUpdate::Remove(id))
        .await
}

#[get("/api/allowedUsers", session: auth::Session)]
pub(crate) async fn get_allowed_users() -> Result<Vec<String>, anyhow::Error> {
    if !session.is_admin {
        return Err(anyhow::anyhow!("Unauthorized"));
    }
    DB.get_allowed_users()
        .map_err(|e| anyhow::anyhow!("Failed to get allowed users: {}", e))
}

#[post("/api/allowedUsers/add", session: auth::Session)]
pub(crate) async fn add_allowed_user(email: String) -> Result<(), anyhow::Error> {
    if !session.is_admin {
        return Err(anyhow::anyhow!("Unauthorized"));
    }
    DB.add_allowed_user(&email)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add allowed user: {}", e))
}

#[post("/api/allowedUsers/remove", session: auth::Session)]
pub(crate) async fn remove_allowed_user(email: String) -> Result<(), anyhow::Error> {
    if !session.is_admin {
        return Err(anyhow::anyhow!("Unauthorized"));
    }
    DB.remove_allowed_user(&email)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to remove allowed user: {}", e))
}

#[post("/api/allowedUsers/clear", session: auth::Session)]
pub(crate) async fn clear_allowed_users() -> Result<(), anyhow::Error> {
    if !session.is_admin {
        return Err(anyhow::anyhow!("Unauthorized"));
    }
    DB.clear_allowed_users()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to clear allowed users: {}", e))
}
