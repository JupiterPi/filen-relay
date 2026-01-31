use dioxus::prelude::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
}

#[derive(Clone)]
struct AuthToken(Signal<Option<String>>);

#[component]
pub(crate) fn App() -> Element {
    use_context_provider(|| AuthToken(Signal::new(None)));
    rsx! {
        document::Title { "Filen Relay" }
        document::Link { rel: "icon", href: "https://filen.io/favicon.ico" }
        document::Link { rel: "stylesheet", href: asset!("/assets/main.css") }
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        Router::<Route> {}
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        h1 { "Welcome to Filen Relay" }
        LoggedIn {}
        LoginForm {}
    }
}

#[component]
fn LoggedIn() -> Element {
    let auth_token = use_context::<AuthToken>().0;
    rsx! {
        if let Some(token) = auth_token() {
            div { "Logged in with token: {token}" }
        } else {
            div { "Not logged in" }
        }
    }
}

#[component]
fn LoginForm() -> Element {
    let mut auth_token = use_context::<AuthToken>().0;
    let mut email = use_signal(|| "".to_string());
    let mut password = use_signal(|| "".to_string());

    rsx! {
        form {
            onsubmit: move |e| async move {
                e.prevent_default();
                match crate::api::login(email.cloned(), password.cloned()).await {
                    Ok(response) => {
                        println!("Login successful, token: {}", response.token);
                        auth_token.set(Some(response.token));
                    }
                    Err(err) => {
                        println!("Login failed: {}", err);
                    }
                };
            },
            div {
                label { "Email:" }
                input {
                    r#type: "email",
                    value: "{email}",
                    oninput: move |e| email.set(e.value().clone()),
                }
            }
            div {
                label { "Password:" }
                input {
                    r#type: "password",
                    value: "{password}",
                    oninput: move |e| password.set(e.value().clone()),
                }
            }
            button { r#type: "submit", "Login" }
        }
    }
}

#[component]
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
            }
        }

        Outlet::<Route> {}
    }
}
