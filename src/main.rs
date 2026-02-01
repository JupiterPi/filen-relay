mod api;
mod db;
mod frontend;

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(frontend::App);

    #[cfg(feature = "server")]
    api::serve();
}
