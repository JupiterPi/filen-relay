mod api;
mod common;
#[cfg(feature = "server")]
mod db;
mod frontend;
#[cfg(feature = "server")]
mod servers;

#[cfg(feature = "server")]
//#[tokio::main]
/* async */
fn main() {
    api::serve();
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(frontend::App);
}
