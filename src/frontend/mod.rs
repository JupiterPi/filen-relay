mod manage_allowed_users;
mod servers;
use std::ops::Deref;

use dioxus::{
    logger::tracing::{self},
    prelude::*,
};

use crate::frontend::{
    manage_allowed_users::ManageAllowedUsers,
    servers::{CreateServerForm, Logs, Servers},
};

struct Authentication {
    pub email: String,
    pub is_admin: bool,
}
static AUTH: GlobalSignal<Option<Authentication>> = Signal::global(|| None);

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub(crate) enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/login")]
    Login {},
    #[route("/logs/:logs_id")]
    LogsPage { logs_id: String },
    #[route("/manage-allowed-users")]
    ManageAllowedUsersPage {},
}

#[component]
fn Navbar() -> Element {
    rsx! {
        div { id: "navbar", class: "flex gap-4 border-b-1 border-gray-400 p-4",
            Link { to: Route::Home {}, class: "font-bold", "Filen Relay" }
            div { class: "flex-1" }
            if let Some(auth) = AUTH.read().deref() {
                span { "{auth.email}" }
            } else {
                Link { to: Route::Login {}, "Login" }
            }
        }
        div { class: "p-4", Outlet::<Route> {} }
    }
}

#[component]
fn Login() -> Element {
    let mut email = use_signal(|| "".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut two_factor_code = use_signal(|| None::<String>);

    let mut loading = use_signal(|| false);

    rsx! {
        div { class: "w-full flex justify-center",
            form {
                class: "flex flex-col gap-2",
                onsubmit: move |e| async move {
                    e.prevent_default();
                    loading.set(true);
                    match crate::api::login(
                            email.cloned(),
                            password.cloned(),
                            two_factor_code.cloned(),
                        )
                        .await
                    {
                        Ok(_response) => {
                            tracing::info!("Logged in successfully");
                            match crate::api::get_user().await {
                                Ok(user) => {
                                    *AUTH.write() = Some(Authentication {
                                        email: user.email,
                                        is_admin: user.is_admin,
                                    });
                                }
                                Err(_) => {
                                    tracing::error!("Failed to fetch user info after login");
                                }
                            }
                            email.set("".to_string());
                            password.set("".to_string());
                            two_factor_code.set(None);
                        }
                        Err(err) => {
                            tracing::error!("Login failed: {}", err);
                        }
                    };
                    loading.set(false);
                    navigator().push(Route::Home {});
                },
                div {
                    label { "Email:" }
                    input {
                        class: "_input w-full",
                        r#type: "email",
                        value: "{email}",
                        oninput: move |e| email.set(e.value().clone()),
                    }
                }
                div {
                    label { "Password:" }
                    input {
                        class: "_input w-full",
                        r#type: "password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value().clone()),
                    }
                }
                div {
                    label { "2FA Code (optional):" }
                    input {
                        class: "_input w-full",
                        r#type: "text",
                        value: format!("{}", two_factor_code().as_deref().unwrap_or("")),
                        oninput: move |e| {
                            let val = e.value().clone();
                            if val.is_empty() {
                                two_factor_code.set(None);
                            } else {
                                two_factor_code.set(Some(val));
                            }
                        },
                    }
                }
                button {
                    class: "_button",
                    disabled: *loading.read(),
                    r#type: "submit",
                    "Login"
                }
            }
        }
    }
}

#[component]
pub(crate) fn App() -> Element {
    spawn(async move {
        match crate::api::get_user().await {
            Ok(user) => {
                tracing::info!("Authenticated as {}", user.email);
                *AUTH.write() = Some(Authentication {
                    email: user.email,
                    is_admin: user.is_admin,
                });
            }
            Err(err) => {
                tracing::info!("Not authenticated: {}", err);
            }
        }
    });

    rsx! {
        document::Title { "Filen Relay" }
        document::Link { rel: "icon", href: "https://filen.io/favicon.ico" }
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        Router::<Route> {}
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        if let Some(auth) = AUTH.read().deref() {
            div { class: "flex flex-col gap-4",
                div { class: "italic",
                    "Welcome to Filen Relay, {auth.email}!"
                    if auth.is_admin {
                        span { class: "text-red-500 italic", " You have admin privileges." }
                    }
                }
                Servers {}
                CreateServerForm {}
                if auth.is_admin {
                    Link {
                        to: Route::ManageAllowedUsersPage {},
                        class: "_button",
                        "Manage Allowed Users"
                    }
                }
            }
        } else {
            div { class: "italic", "Welcome to Filen Relay! Please log in to continue." }
        }
    }
}

#[component]
fn LogsPage(logs_id: String) -> Element {
    rsx! {
        Logs { logs_id }
    }
}

#[component]
fn ManageAllowedUsersPage() -> Element {
    rsx! {
        ManageAllowedUsers {}
    }
}
