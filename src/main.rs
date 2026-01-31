mod api;
mod db;
mod frontend;

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(frontend::App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move { Ok(dioxus::server::router(frontend::App)) })
}
