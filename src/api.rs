use crate::common::{LogLine, ServerState, ServerType};
#[cfg(feature = "server")]
use crate::servers::SERVER_MANAGER;
use anyhow::Context;
use dioxus::{
    fullstack::{response::Response, JsonEncoding, Streaming},
    prelude::*,
};
#[cfg(feature = "server")]
use filen_sdk_rs::auth::Client;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
mod session {
    use dioxus::{
        fullstack::extract::{FromRequestParts, Request},
        prelude::*,
        server::{
            axum::{self, middleware::Next},
            http::request::Parts,
        },
    };
    use std::sync::{LazyLock, Mutex};

    static SESSIONS: LazyLock<Mutex<Vec<Session>>> = LazyLock::new(|| Mutex::new(Vec::new()));

    #[derive(Clone)]
    pub struct SessionToken(String);

    #[derive(Clone)]
    pub(crate) struct Session {
        pub token: String,
        pub filen_email: String,
        pub filen_password: String,
        pub filen_2fa_code: Option<String>,
    }

    pub(crate) async fn extract_session_token(
        mut request: Request,
        next: Next,
    ) -> axum::http::Response<axum::body::Body> {
        if let Some(cookies) = request.headers().get("Cookie") {
            let token = cookies
                .to_str()
                .unwrap_or("")
                .split(';')
                .find_map(|cookie| {
                    let (name, value) = cookie.trim().split_once('=')?;
                    if name == "Session" {
                        Some(value.to_string())
                    } else {
                        None
                    }
                });
            if let Some(token) = token {
                request.extensions_mut().insert(SessionToken(token));
            }
        }
        next.run(request).await
    }

    impl<S> FromRequestParts<S> for Session
    where
        S: Send + Sync,
    {
        type Rejection = StatusCode;

        async fn from_request_parts(
            parts: &mut Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            parts
                .extensions
                .get::<SessionToken>()
                .and_then(|token| {
                    SESSIONS
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|s| s.token == token.0)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("Invalid session token"))
                        .ok()
                })
                .ok_or(StatusCode::UNAUTHORIZED)
        }
    }

    pub(crate) fn create_session(
        filen_email: &str,
        filen_password: &str,
        filen_2fa_code: Option<String>,
    ) -> Result<String, anyhow::Error> {
        let token = uuid::Uuid::new_v4().to_string();
        SESSIONS.lock().unwrap().push(Session {
            token: token.clone(),
            filen_email: filen_email.to_string(),
            filen_password: filen_password.to_string(),
            filen_2fa_code,
        });
        Ok(token)
    }
}

#[cfg(feature = "server")]
pub(crate) fn serve() {
    dioxus::serve(|| async move {
        SERVER_MANAGER.init(crate::servers::ServerManager::new_api);

        Ok(dioxus::server::router(crate::frontend::App).layer(
            dioxus_server::axum::middleware::from_fn(session::extract_session_token),
        ))
    });
}

#[derive(Serialize, Deserialize)]
pub(crate) struct User {
    pub email: String,
}

#[post("/api/user", session: session::Session)]
pub(crate) async fn get_user() -> Result<User> {
    Ok(User {
        email: session.filen_email,
    })
}

#[cfg(feature = "server")]
pub(crate) async fn authenticate_filen_client(
    email: String,
    password: &str,
    two_factor_code: Option<String>,
) -> Result<Client, anyhow::Error> {
    use filen_sdk_rs::ErrorKind;
    use filen_types::error::ResponseError;
    match Client::login(
        email.clone(),
        password,
        two_factor_code.as_deref().unwrap_or("XXXXXX"),
    )
    .await
    {
        Err(e) if e.kind() == ErrorKind::Server => match e.downcast::<ResponseError>() {
            Ok(ResponseError::ApiError { code, .. }) => {
                if code.as_deref() == Some("enter_2fa") {
                    Err(anyhow::anyhow!("2FA required"))
                } else if code.as_deref() == Some("email_or_password_wrong") {
                    Err(anyhow::anyhow!("Email or password wrong"))
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to log in (code {})",
                        code.as_deref().unwrap_or("")
                    ))
                }
            }
            Err(e) => Err(anyhow::anyhow!(e)).context("Failed to log in"),
        },
        Err(e) => Err(anyhow::anyhow!(e)).context("Failed to log in"),
        Ok(client) => Ok(client),
    }
}

#[post("/api/login")]
pub(crate) async fn login(
    email: String,
    password: String,
    two_factor_code: Option<String>,
) -> Result<Response, anyhow::Error> {
    match authenticate_filen_client(email.clone(), &password, two_factor_code.clone()).await {
        Err(e) => Err(anyhow::anyhow!(e)).context("Failed to log in"),
        Ok(_client) => {
            let allowed_users = crate::db::get_allowed_users()
                .map_err(|e| anyhow::anyhow!("Failed to get allowed users from database: {}", e))?;
            let is_allowed = if allowed_users.is_empty() {
                true
            } else {
                allowed_users.contains(&email)
            };
            if is_allowed {
                use dioxus::fullstack::{body::Body, response::Response};

                let token = session::create_session(&email, &password, two_factor_code.clone())?;
                Ok(Response::builder()
                    .header("Set-Cookie", format!("Session={}; HttpOnly; Path=/", token))
                    .body(Body::empty())
                    .unwrap())
            } else {
                Err(anyhow::anyhow!("User is not allowed"))
            }
        }
    }
}

#[get("/api/servers", session: session::Session)]
pub(crate) async fn get_servers() -> Result<Streaming<Vec<ServerState>, JsonEncoding>> {
    Ok(Streaming::spawn(|tx| async move {
        let send_server_states = || {
            let server_states = SERVER_MANAGER
                .get_server_states()
                .borrow()
                .iter()
                .filter(|s| s.spec.filen_email == session.filen_email)
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

#[get("/api/logs/{logs_id}", session: session::Session)]
pub(crate) async fn get_logs(logs_id: String) -> Result<Streaming<LogLine, JsonEncoding>> {
    let Some(logs) = SERVER_MANAGER.get_logs(&logs_id) else {
        return Err(anyhow::anyhow!("Logs not found"))?;
    };
    if logs.server_spec.filen_email != session.filen_email {
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

#[post("/api/servers/add", session: session::Session)]
pub(crate) async fn add_server(name: String, server_type: ServerType) -> Result<(), anyhow::Error> {
    SERVER_MANAGER
        .update_server_spec(crate::servers::ServerSpecUpdate::Add {
            name,
            server_type,
            filen_email: session.filen_email,
            filen_password: session.filen_password,
            filen_2fa_code: session.filen_2fa_code,
        })
        .await
}

#[post("/api/servers/remove", session: session::Session)]
pub(crate) async fn remove_server(id: String) -> Result<(), anyhow::Error> {
    SERVER_MANAGER
        .get_server_states()
        .borrow()
        .iter()
        .find(|s| s.spec.id == id && s.spec.filen_email == session.filen_email)
        .ok_or_else(|| anyhow::anyhow!("Server not found or not owned by user"))?;
    SERVER_MANAGER
        .update_server_spec(crate::servers::ServerSpecUpdate::Remove(id))
        .await
}
