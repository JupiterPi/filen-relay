use dioxus::server::axum;

use crate::{
    backend::{
        auth::ADMIN_EMAIL,
        db::{DbViaOfflineOrRemoteFile, DB},
        server_manager::{ServerManager, SERVER_MANAGER},
    },
    Args,
};

pub(crate) mod auth;
pub(crate) mod db;
pub(crate) mod server_manager;

pub(crate) fn serve(args: Args) {
    dioxus::serve(move || {
        let args = args.clone();
        async move {
            let (admin_email, db) = match (
                    args.admin_email,
                    args.admin_password,
                    args.admin_2fa_code,
                    args.admin_auth_config,
                    args.db_dir,
                ) {
                    (Some(email), _, _, _, Some(db_dir)) => {
                        let db = DbViaOfflineOrRemoteFile::new_from_offline_location(Some(&db_dir)).await;
                        db.map(|db| (email, db))
                    }
                    (_, _, _, Some(auth_config), _) => {
                        DbViaOfflineOrRemoteFile::new_from_auth_config(auth_config).await
                    }
                    (Some(email), Some(password), two_fa_code, _, _) => {
                        let db = DbViaOfflineOrRemoteFile::new_from_email_and_password(
                            email.clone(),
                            &password,
                            two_fa_code.as_deref(),
                        )
                        .await;
                        db.map(|db| (email, db))
                    }
                    _ => panic!(
                        "Either admin email and local db dir, email/password or auth config must be provided"
                    ),
                }.expect("Failed to initialize database");
            ADMIN_EMAIL.set(admin_email).unwrap();
            DB.init(db);

            use axum_reverse_proxy::ProxyRouterExt;

            SERVER_MANAGER.init(ServerManager::new_api());

            Ok(dioxus::server::router(crate::frontend::App)
                .layer(axum::middleware::from_fn(
                    auth::middleware_extract_session_token,
                ))
                .proxy_route(
                    "/s/{id}",
                    ServerResolver {
                        with_rest: false,
                        append_slash: false,
                    },
                )
                .proxy_route(
                    "/s/{id}/",
                    ServerResolver {
                        with_rest: false,
                        append_slash: true,
                    },
                )
                .proxy_route(
                    "/s/{id}/{*rest}",
                    ServerResolver {
                        with_rest: true,
                        append_slash: false,
                    },
                ))
        }
    });
}

#[derive(Clone)]
struct ServerResolver {
    with_rest: bool,
    append_slash: bool,
}

impl axum_reverse_proxy::TargetResolver for ServerResolver {
    fn resolve(
        &self,
        _req: &axum::http::Request<axum::body::Body>,
        params: &[(String, String)],
    ) -> String {
        let id = params[0].1.as_str();
        if id.len() < 4 {
            return "https://postman-echo.com/get/status/404".to_string();
        }
        let rest = if self.with_rest {
            "/".to_string() + params.get(1).map(|(_, v)| v.as_str()).unwrap_or("")
        } else {
            "".to_string()
        };
        let server_states = SERVER_MANAGER.get_server_states().borrow().clone();
        let Some(server_state) = server_states.iter().find(|s| s.spec.id.short() == id) else {
            return "https://postman-echo.com/get/status/404".to_string();
        };
        let crate::common::ServerStatus::Running { port, .. } = server_state.status else {
            return "https://postman-echo.com/get/status/404".to_string();
        };
        let extra_slash = if self.append_slash { "/" } else { "" };
        format!("http://127.0.0.1:{}{}{}", port, rest, extra_slash)
    }
}
