mod api;
mod common;
#[cfg(feature = "server")]
mod db;
mod frontend;
#[cfg(feature = "server")]
mod servers;
mod util;

#[cfg(feature = "server")]
#[derive(clap::Parser)]
#[command(version)]
struct Args {
    #[arg(
        long,
        env = "FILEN_RELAY_ADMIN_EMAIL",
        help = "Email of the Filen account with admin privileges"
    )]
    admin_email: String,
    #[arg(
        long,
        env = "FILEN_RELAY_DB_DIR",
        help = "Directory to store the database file"
    )]
    db_dir: Option<String>,
}

#[cfg(feature = "server")]
fn main() {
    let args = <Args as clap::Parser>::parse();
    api::serve(args.admin_email, args.db_dir);
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(frontend::App);
}
