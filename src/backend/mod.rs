use dioxus::server::axum;

use crate::backend::{
    auth::ADMIN_EMAIL,
    db::DB_DIR,
    server_manager::{ServerManager, SERVER_MANAGER},
};

pub(crate) mod auth;
pub(crate) mod db;
pub(crate) mod server_manager;

pub(crate) fn serve(admin_email: String, db_dir: Option<String>) {
    ADMIN_EMAIL.set(admin_email).unwrap();
    DB_DIR.set(db_dir).unwrap();
    dioxus::serve(|| async move {
        use axum_reverse_proxy::ProxyRouterExt;

        SERVER_MANAGER.init(ServerManager::new_api);

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
