use std::ops::Deref;

use dioxus::{
    logger::tracing::{self},
    prelude::*,
};
use strum::IntoEnumIterator as _;

use crate::common::ServerType;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/login")]
    Login {},
}

struct Authentication {
    pub email: String,
}
static AUTH: GlobalSignal<Option<Authentication>> = Signal::global(|| None);

#[component]
pub(crate) fn App() -> Element {
    spawn(async move {
        match crate::api::get_user().await {
            Ok(user) => {
                tracing::info!("Authenticated as {}", user.email);
                *AUTH.write() = Some(Authentication { email: user.email });
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
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            class: "flex gap-4 border-b-1 border-gray-400 p-4",
            Link {
                to: Route::Home {},
                class: "font-bold",
                "Filen Relay"
            }
            div {
                class: "flex-1",
            }
            if let Some(auth) = AUTH.read().deref() {
                span { "{auth.email}" }
            } else {
                Link {
                    to: Route::Login {},
                    "Login"
                }
            }
        }
        div {
            class: "p-4",
            Outlet::<Route> {}
        }
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        if let Some(auth) = AUTH.read().deref() {
            div {
                class: "italic mb-3",
                "Welcome to Filen Relay, {auth.email}!"
            }
            Servers {}
            CreateServerForm {}
        } else {
            div {
                class: "italic",
                "Welcome to Filen Relay! Please log in to continue."
            }
        }
    }
}

#[component]
fn Login() -> Element {
    let mut email = use_signal(|| "".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut two_factor_code = use_signal(|| None::<String>);

    let mut loading = use_signal(|| false);

    rsx! {
        div {
            class: "w-full flex justify-center",
            form {
                class: "flex flex-col gap-2",
                onsubmit: move |e| async move {
                    e.prevent_default();
                    loading.set(true);
                    match crate::api::login(email.cloned(), password.cloned(), two_factor_code.cloned()).await {
                        Ok(_response) => {
                            tracing::info!("Logged in successfully");
                            *AUTH.write() = Some(Authentication {
                                email: email.to_string(),
                            });
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
fn Servers() -> Element {
    let servers = use_resource(move || async move { crate::api::get_servers().await });
    let servers = &*servers.read();
    match servers {
        Some(Ok(servers)) => rsx! {
            div {
                class: "flex flex-wrap gap-4",
                for server in servers.clone() {
                    div {
                        class: "border p-4 inline-flex flex-col w-64",
                        h2 { class: "font-bold text-lg", "{server.spec.name}" }
                        p { "Type: {server.spec.server_type}" }
                        p { "Status: {server.status}" }
                        button {
                            class: "_button mt-2",
                            onclick: move |_| async move {
                                match crate::api::remove_server(server.spec.id).await {
                                    Ok(_) => {
                                        tracing::info!("Server removed successfully");
                                    },
                                    Err(err) => {
                                        tracing::error!("Failed to remove server: {}", err);
                                    },
                                };
                            },
                            "Remove Server"
                        }
                    }
                }
            }
            if servers.is_empty() {
                div { "No servers available." }
            }
        },
        Some(Err(e)) => rsx! {
            div {
                class: "text-red-500",
                "Failed to load servers: {e}"
            }
        },
        None => rsx! {
            div { "Loading servers..." }
        },
    }
}

#[component]
fn CreateServerForm() -> Element {
    let mut name = use_signal(|| "".to_string());
    let mut server_type = use_signal(|| ServerType::Http);

    rsx! {
        form {
            class: "flex flex-col gap-2",
            onsubmit: move |e| async move {
                e.prevent_default();
                match crate::api::add_server(name.read().clone(), server_type.read().cloned()).await {
                    Ok(_) => {
                        tracing::info!("Server created successfully");
                    },
                    Err(err) => {
                        tracing::error!("Failed to create server: {}", err);
                    },
                };
            },
            div {
                label { "Server Name:" }
                input {
                    class: "_input w-full",
                    r#type: "text",
                    value: "{name}",
                    oninput: move |e| name.set(e.value().clone()),
                }
            }
            div {
                label { "Server Type:" }
                select {
                    class: "_input w-full",
                    onchange: move |e| {
                        server_type.set(ServerType::from(e.value().as_str()));
                    },
                    for server_type in ServerType::iter() {
                        option {
                            value: server_type.to_string(),
                            "{server_type.to_string()}"
                        }
                    }
                }
            }
            button {
                class: "_button",
                r#type: "submit",
                "Create Server"
            }
        }
    }
}
