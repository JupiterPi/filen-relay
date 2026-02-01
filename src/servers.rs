use anyhow::{Context, Result};
use dioxus::logger::tracing;
use tokio::sync::watch;

use crate::common::ServerSpec;
use crate::common::ServerState;
use crate::common::ServerStatus;
use crate::common::ServerType;
use crate::util::UnwrapOnceLock;

pub(crate) static SERVER_MANAGER: UnwrapOnceLock<ServerManager> =
    UnwrapOnceLock::<ServerManager>::new();

pub(crate) struct ServerManager {
    server_states_rx: tokio::sync::watch::Receiver<Vec<ServerState>>,
    updates_tx: tokio::sync::mpsc::Sender<ServerSpecUpdate>,
}

pub(crate) enum ServerSpecUpdate {
    Add {
        name: String,
        server_type: ServerType,
        filen_email: String,
        filen_password: String,
    },
    Remove(i32),
}

impl ServerManager {
    pub(crate) fn new() -> Self {
        let (server_states_tx, server_states_rx) =
            tokio::sync::watch::channel(Vec::<ServerState>::new());
        let (updates_tx, mut updates_rx) = tokio::sync::mpsc::channel::<ServerSpecUpdate>(100);

        tokio::spawn(async move {
            let servers = match crate::db::get_servers() {
                Ok(servers) => servers,
                Err(e) => {
                    tracing::error!("Failed to load server specs from database: {}", e);
                    return;
                }
            };
            for server in servers {
                if let Err(e) = Self::start_server(&server, server_states_tx.clone()).await {
                    tracing::error!("Failed to start server {}: {}", server.name, e);
                }
            }

            loop {
                if let Some(update) = updates_rx.recv().await {
                    match update {
                        ServerSpecUpdate::Add {
                            name,
                            server_type,
                            filen_email,
                            filen_password,
                        } => {
                            tracing::info!("Adding server spec: {}", name);
                            let spec = match crate::db::create_server(
                                &name,
                                server_type,
                                &filen_email,
                                &filen_password,
                            ) {
                                Ok(spec) => spec,
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to create server spec in database: {}",
                                        e
                                    );
                                    continue;
                                }
                            };
                            Self::start_server(&spec, server_states_tx.clone())
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::error!("Failed to start server: {}", e);
                                });
                        }
                        ServerSpecUpdate::Remove(id) => {
                            let spec = {
                                let states = server_states_tx.borrow();
                                match states.iter().find(|s| s.spec.id == id) {
                                    Some(s) => s.spec.clone(),
                                    None => {
                                        tracing::error!("Server spec with id {} not found", id);
                                        continue;
                                    }
                                }
                            };
                            match crate::db::delete_server(id) {
                                Ok(_) => (),
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to delete server spec from database: {}",
                                        e
                                    );
                                    continue;
                                }
                            };
                            tracing::info!("Removing server spec with id: {}", id);
                            Self::stop_server(&spec, server_states_tx.clone())
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::error!("Failed to stop server: {}", e);
                                });
                        }
                    }
                } else {
                    tracing::error!("Server spec updates channel closed");
                    break;
                }
            }
        });

        Self {
            updates_tx,
            server_states_rx,
        }
    }

    async fn start_server(
        spec: &ServerSpec,
        server_states_tx: watch::Sender<Vec<ServerState>>,
    ) -> Result<()> {
        // todo: implement operation
        server_states_tx.send_modify(|server_states| {
            server_states.push(ServerState {
                spec: spec.clone(),
                status: ServerStatus::Stopped,
            });
        });
        Ok(())
    }

    async fn stop_server(
        spec: &ServerSpec,
        server_states_tx: watch::Sender<Vec<ServerState>>,
    ) -> Result<()> {
        // todo: implement operation
        server_states_tx.send_modify(|server_states| {
            server_states.retain(|s| s.spec.id != spec.id);
        });
        Ok(())
    }

    /// Returns a receiver to listen for server state updates.
    pub(crate) fn get_server_states(&self) -> tokio::sync::watch::Receiver<Vec<ServerState>> {
        self.server_states_rx.clone()
    }

    /// Add/remove the server spec via the manager (will start/stop it) and persist it to the database.
    pub(crate) async fn update_server_spec(&self, update: ServerSpecUpdate) -> Result<()> {
        self.updates_tx
            .send(update)
            .await
            .context("Failed to send server spec update")
    }
}
