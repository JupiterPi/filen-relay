use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db;

#[derive(Serialize, Deserialize)]
pub(crate) struct LoginResponse {
    pub token: String,
}

#[post("/api/login")]
pub(crate) async fn login(email: String, password: String) -> Result<LoginResponse> {
    let allowed_users = db::conn::get_allowed_users()?;
    let is_allowed = if allowed_users.is_empty() {
        true
    } else {
        allowed_users.contains(&email)
    };
    if is_allowed {
        Ok(LoginResponse {
            token: "dummy_token".to_string(),
        })
        // todo: tmp
    } else {
        Err(ServerFnError::new("User is not allowed").into())
    }
}
