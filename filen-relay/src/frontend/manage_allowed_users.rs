use dioxus::{core::Element, hooks::use_signal, prelude::component};
use dioxus::{
    logger::tracing::{self},
    prelude::*,
};

#[component]
pub(crate) fn ManageAllowedUsers() -> Element {
    let mut allowed_users = use_signal(|| None::<Vec<String>>);
    let mut loading = use_signal(|| false);
    let mut new_user_email = use_signal(|| "".to_string());

    let fetch_users = move || {
        spawn(async move {
            loading.set(true);
            match crate::api::get_allowed_users().await {
                Ok(users) => {
                    allowed_users.set(Some(users));
                }
                Err(err) => {
                    tracing::error!("Failed to fetch allowed users: {}", err);
                }
            }
            loading.set(false);
        });
    };
    use_effect(move || {
        fetch_users();
    });

    rsx! {
        div { class: "flex flex-col gap-4 border p-4 rounded-lg",
            h2 { class: "font-bold text-lg", "Manage Allowed Users" }
            form {
                class: "flex gap-2 items-center",
                onsubmit: move |e| async move {
                    e.prevent_default();
                    let email = new_user_email.read().clone();
                    if email.is_empty() {
                        tracing::error!("Email cannot be empty");
                        return;
                    }
                    match crate::api::add_allowed_user(email).await {
                        Ok(_) => {
                            tracing::info!("User added successfully");
                            new_user_email.set("".to_string());
                            fetch_users();
                        }
                        Err(err) => {
                            tracing::error!("Failed to add user: {}", err);
                        }
                    }
                },
                input {
                    class: "_input flex-1",
                    r#type: "email",
                    placeholder: "user@example.com",
                    value: "{new_user_email}",
                    oninput: move |e| new_user_email.set(e.value().clone()),
                }
                button {
                    class: "_button",
                    r#type: "submit",
                    disabled: new_user_email.read().is_empty(),
                    "Add User"
                }
            }
            if *loading.read() {
                div { class: "text-gray-500", "Loading..." }
            } else {
                match allowed_users() {
                    Some(users) if !users.is_empty() => rsx! {
                        div { class: "flex flex-col gap-2",
                            for user in users.iter().cloned() {
                                div { class: "flex items-center gap-2 p-2 border rounded",
                                    span { class: "flex-1", "{user}" }
                                    button {
                                        class: "_button px-2 py-1 text-sm bg-red-500 hover:bg-red-600",
                                        onclick: move |_| {
                                            let user = user.clone();
                                            async move {
                                                match crate::api::remove_allowed_user(user.clone()).await {
                                                    Ok(_) => {
                                                        tracing::info!("User removed successfully");
                                                        fetch_users();
                                                    }
                                                    Err(err) => {
                                                        tracing::error!("Failed to remove user: {}", err);
                                                    }
                                                }
                                            }
                                        },
                                        "✕"
                                    }
                                }
                            }
                            button {
                                class: "_button mt-2 bg-red-500 hover:bg-red-600",
                                onclick: move |_| async move {
                                    match crate::api::clear_allowed_users().await {
                                        Ok(_) => {
                                            tracing::info!("All users cleared successfully");
                                            fetch_users();
                                        }
                                        Err(err) => {
                                            tracing::error!("Failed to clear users: {}", err);
                                        }
                                    }
                                },
                                "Clear All"
                            }
                        }
                    },
                    Some(_) => rsx! {
                        div { class: "text-red-500",
                            "No allowed users configured. This means that anyone is allowed to access the system and create servers."
                        }
                    },
                    None => rsx! {
                        div { class: "text-gray-500", "Failed to load users." }
                    },
                }
            }
        }
    }
}
