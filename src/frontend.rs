use std::ops::Deref;

use dioxus::{
    logger::tracing::{self},
    prelude::*,
};
use strum::IntoEnumIterator as _;

use crate::common::{ServerState, ServerStatus, ServerType};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/login")]
    Login {},
    #[route("/logs/:logs_id")]
    Logs { logs_id: String },
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
fn Home() -> Element {
    rsx! {
        if let Some(auth) = AUTH.read().deref() {
            div { class: "flex flex-col gap-4",
                div { class: "italic", "Welcome to Filen Relay, {auth.email}!" }
                Servers {}
                CreateServerForm {}
            }
        } else {
            div { class: "italic", "Welcome to Filen Relay! Please log in to continue." }
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
    let mut servers = use_signal(|| None::<Vec<ServerState>>);
    use_future(move || async move {
        match crate::api::get_servers().await {
            Ok(mut servers_stream) => loop {
                match servers_stream.next().await {
                    Some(Ok(new_servers)) => {
                        servers.set(Some(new_servers));
                    }
                    Some(Err(err)) => {
                        tracing::error!("Error receiving server states: {}", err);
                        break;
                    }
                    None => {
                        tracing::info!("Server states stream ended");
                        break;
                    }
                }
            },
            Err(err) => {
                tracing::error!("Failed to fetch servers: {}", err);
            }
        }
    });
    let servers = &*servers;

    match servers() {
        Some(servers) if !servers.is_empty() => {
            rsx! {
                div { class: "flex flex-wrap gap-4",
                    for (server , server_id_short) in servers.iter().map(|s| (s.clone(), s.spec.id.split_once('-').unwrap().0)) {
                        div { class: "border p-4 inline-flex flex-col w-64 rounded-lg",
                            h2 { class: "font-bold text-lg", "{server.spec.name}" }
                            p {
                                "ID: "
                                span { class: "font-mono", "#{server_id_short}" }
                            }
                            p { "Type: {server.spec.server_type}" }
                            match server.status.clone() {
                                ServerStatus::Starting => rsx! {
                                    p { class: "text-gray-500", "Status: Starting..." }
                                },
                                ServerStatus::Running { connection_url } => rsx! {
                                    p { class: "text-green-500", "Online" }
                                    a { href: "{connection_url}", target: "_blank", "{connection_url}" }
                                },
                                ServerStatus::Error => rsx! {
                                    p { class: "text-red-500", "Status: Error" }
                                },
                            }
                            Link {
                                to: Route::Logs {
                                    logs_id: server.logs_id.clone(),
                                },
                                class: "flex _button mt-2",
                                "View Logs"
                            }
                            button {
                                class: "_button mt-2",
                                onclick: move |_| {
                                    let server = server.clone();
                                    async move {
                                        match crate::api::remove_server(server.spec.id.clone()).await {
                                            Ok(_) => {
                                                tracing::info!("Server removed successfully");
                                            }
                                            Err(err) => {
                                                tracing::error!("Failed to remove server: {}", err);
                                            }
                                        };
                                    }
                                },
                                "Remove Server"
                            }
                        }
                    }
                }
            }
        }
        Some(_) => {
            rsx! {
                div { class: "text-gray-500", "No servers available." }
            }
        }
        None => rsx! {
            div { class: "text-gray-500", "Loading servers..." }
        },
    }
}

#[component]
fn CreateServerForm() -> Element {
    let mut name = use_signal(|| "".to_string());
    let mut server_type = use_signal(|| ServerType::Http);

    rsx! {
        form {
            class: "flex flex-col gap-2 border p-4 rounded-lg",
            onsubmit: move |e| async move {
                e.prevent_default();
                let name_ = name.read().clone();
                if name_.is_empty() {
                    tracing::error!("Server name cannot be empty");
                    return;
                }
                let server_type_ = server_type.read().clone();
                match crate::api::add_server(name_.to_string(), server_type_.clone()).await {
                    Ok(_) => {
                        tracing::info!("Server created successfully");
                        name.set("".to_string());
                        server_type.set(ServerType::Http);
                    }
                    Err(err) => {
                        tracing::error!("Failed to create server: {}", err);
                    }
                };
            },
            div { class: "flex items-stretch gap-2",
                div {
                    label { "Server Name:" }
                    input {
                        class: "mt-1 _input",
                        r#type: "text",
                        placeholder: "My Server",
                        value: "{name}",
                        oninput: move |e| name.set(e.value().clone()),
                    }
                }
                div {
                    label { "Server Type:" }
                    select {
                        class: "mt-1 _input w-full",
                        onchange: move |e| {
                            server_type.set(ServerType::from(e.value().as_str()));
                        },
                        for server_type in ServerType::iter() {
                            option { value: server_type.to_string(), "{server_type.to_string()}" }
                        }
                    }
                }
            }
            button {
                class: "_button",
                r#type: "submit",
                disabled: name.read().is_empty(),
                "Create Server"
            }
        }
    }
}

#[component]
fn Logs(logs_id: String) -> Element {
    let mut logs = use_signal(Vec::<String>::new);
    use_future(move || {
        let logs_id = logs_id.clone();
        async move {
            match crate::api::get_logs(logs_id.clone()).await {
                Ok(mut logs_stream) => loop {
                    match logs_stream.next().await {
                        Some(Ok(new_log)) => {
                            logs.write().push(new_log);
                        }
                        Some(Err(err)) => {
                            tracing::error!("Error receiving logs: {}", err);
                            break;
                        }
                        None => {
                            tracing::info!("Logs stream ended");
                            break;
                        }
                    }
                },
                Err(err) => {
                    tracing::error!("Failed to fetch logs: {}", err);
                }
            }
        }
    });
    rsx! {
        div { class: "flex flex-col gap-2 border p-4 rounded-lg overflow-y-auto font-mono bg-black text-gray-200",
            for log in logs.read().iter() {
                div { "{log}" }
            }
        }
    }
}
