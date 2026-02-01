use anyhow::{anyhow, Context};
use dioxus::{fullstack::response::Response, prelude::*};
use serde::{Deserialize, Serialize};

use crate::db;

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
    ) -> Result<String, anyhow::Error> {
        let token = uuid::Uuid::new_v4().to_string();
        SESSIONS.lock().unwrap().push(Session {
            token: token.clone(),
            filen_email: filen_email.to_string(),
            filen_password: filen_password.to_string(),
        });
        Ok(token)
    }
}

#[cfg(feature = "server")]
pub(crate) fn serve() {
    dioxus::serve(|| async move {
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

#[post("/api/login")]
pub(crate) async fn login(
    email: String,
    password: String,
    two_factor_code: Option<String>,
) -> Result<Response, anyhow::Error> {
    use filen_sdk_rs::{auth::Client, ErrorKind};
    use filen_types::error::ResponseError;

    match Client::login(
        email.clone(),
        &password,
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
            Err(e) => Err(anyhow!(e)).context("Failed to log in"),
        },
        Err(e) => Err(anyhow!(e)).context("Failed to log in"),
        Ok(_client) => {
            let allowed_users = db::conn::get_allowed_users()
                .map_err(|e| anyhow::anyhow!("Failed to get allowed users from database: {}", e))?;
            let is_allowed = if allowed_users.is_empty() {
                true
            } else {
                allowed_users.contains(&email)
            };
            if is_allowed {
                use dioxus::fullstack::{body::Body, response::Response};

                let token = session::create_session(&email, &password)?;
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
pub(crate) async fn get_servers() -> Result<Vec<db::Server>, anyhow::Error> {
    let servers = db::conn::get_servers_for_user(&session.filen_email)
        .map_err(|e| anyhow::anyhow!("Failed to get servers from database: {}", e))?;
    Ok(servers)
}

#[post("/api/servers/create", session: session::Session)]
pub(crate) async fn create_server(r#type: String) -> Result<(), anyhow::Error> {
    db::conn::create_server(
        "New Server",
        &r#type,
        &session.filen_email,
        &session.filen_password,
    )
    .map_err(|e| anyhow::anyhow!("Failed to create server in database: {}", e))?;
    Ok(())
}
