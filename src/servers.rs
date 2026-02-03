use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Context, Result};
use dioxus::logger::tracing;
use filen_rclone_wrapper::rclone_installation::RcloneInstallationConfig;
use filen_rclone_wrapper::serve::BasicServerOptions;
use port_check::free_local_ipv4_port;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::select;
use tokio::sync::oneshot;

use crate::api::authenticate_filen_client;
use crate::common::LogLine;
use crate::common::LogLineContent;
use crate::common::ServerId;
use crate::common::ServerSpec;
use crate::common::ServerState;
use crate::common::ServerStatus;
use crate::common::ServerType;
use crate::util::IncrementalVec;
use crate::util::UnwrapOnceLock;

pub(crate) static SERVER_MANAGER: UnwrapOnceLock<ServerManagerApi> =
    UnwrapOnceLock::<ServerManagerApi>::new();

#[derive(Clone)]
pub(crate) struct Logs {
    pub server_spec: ServerSpec,
    pub logs: Arc<Mutex<IncrementalVec<LogLine>>>,
}

pub(crate) struct ServerManagerApi {
    server_states_rx: tokio::sync::watch::Receiver<Vec<ServerState>>,
    logs: Arc<Mutex<HashMap<String, Logs>>>,
    updates_tx: tokio::sync::mpsc::Sender<ServerSpecUpdate>,
}

pub(crate) enum ServerSpecUpdate {
    Add(ServerSpec),
    Remove(ServerId),
}

type StopServerHandle = oneshot::Sender<()>;

pub(crate) struct ServerManager {
    server_states_tx: tokio::sync::watch::Sender<Vec<ServerState>>,
    logs: Arc<Mutex<HashMap<String, Logs>>>,
    stop_handles: HashMap<ServerId, StopServerHandle>,
}

impl ServerManager {
    pub(crate) fn new_api() -> ServerManagerApi {
        let (server_states_tx, server_states_rx) =
            tokio::sync::watch::channel(Vec::<ServerState>::new());
        let (updates_tx, mut updates_rx) = tokio::sync::mpsc::channel::<ServerSpecUpdate>(100);

        let logs = Arc::new(Mutex::new(HashMap::new()));
        let api = ServerManagerApi {
            updates_tx,
            logs: logs.clone(),
            server_states_rx,
        };
        tokio::spawn(async move {
            Self {
                server_states_tx,
                logs: logs.clone(),
                stop_handles: HashMap::new(),
            }
            .run(&mut updates_rx)
            .await;
        });
        api
    }

    async fn run(mut self, updates_rx: &mut tokio::sync::mpsc::Receiver<ServerSpecUpdate>) {
        // load existing servers from the database and start them
        let servers = match crate::db::get_servers() {
            Ok(servers) => servers,
            Err(e) => {
                tracing::error!("Failed to load server specs from database: {}", e);
                return;
            }
        };
        for server in servers {
            if let Err(e) = self.start_server(&server).await {
                tracing::error!("Failed to start server {}: {}", server.name, e);
            }
        }

        loop {
            // listen for updates
            // on update, persist changes to the database and start/stop servers accordingly
            if let Some(update) = updates_rx.recv().await {
                match update {
                    ServerSpecUpdate::Add(spec) => {
                        tracing::info!("Adding server spec: {}", spec.name);
                        if let Err(e) = crate::db::create_server(&spec) {
                            tracing::error!("Failed to create server spec in database: {}", e);
                            continue;
                        };
                        if let Err(e) = self.start_server(&spec).await {
                            tracing::error!("Failed to start server: {}", e);
                        };
                    }
                    ServerSpecUpdate::Remove(id) => {
                        let spec = {
                            let states = self.server_states_tx.borrow();
                            match states.iter().find(|s| s.spec.id == id) {
                                Some(s) => s.spec.clone(),
                                None => {
                                    tracing::error!("Server spec with id {} not found", id);
                                    continue;
                                }
                            }
                        };
                        match crate::db::delete_server(&id) {
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
                        if let Err(e) = self.stop_server(&spec).await {
                            tracing::error!("Failed to stop server: {}", e);
                        }
                    }
                }
            } else {
                tracing::error!("Server spec updates channel closed");
                break;
            }
        }
    }

    async fn start_server(&mut self, spec: &ServerSpec) -> Result<()> {
        // setup logs
        let logs_id = format!("logs_{}_{}", spec.id.short(), uuid::Uuid::new_v4());
        let logs = {
            let logs = Logs {
                server_spec: spec.clone(),
                logs: Arc::new(Mutex::new(IncrementalVec::<LogLine>::new(100))),
            };
            let logs_ = logs.logs.clone();
            self.logs.lock().unwrap().insert(logs_id.clone(), logs);
            logs_
        };
        let log_info = {
            let logs = logs.clone();
            let spec = spec.clone();
            move |message: &str| {
                logs.lock().unwrap().push(LogLine {
                    timestamp: chrono::Utc::now(),
                    content: LogLineContent::Event(message.to_string()),
                });
                tracing::info!("Server {} ({}): {}", spec.name, spec.id, message);
            }
        };
        let log_err = {
            let logs = logs.clone();
            let spec = spec.clone();
            move |message: &str| {
                logs.lock().unwrap().push(LogLine {
                    timestamp: chrono::Utc::now(),
                    content: LogLineContent::Event(message.to_string()),
                });
                tracing::info!("Server {} ({}) ERR: {}", spec.name, spec.id, message);
            }
        };
        let log_output = {
            let logs = logs.clone();
            move |message: &str| {
                logs.lock().unwrap().push(LogLine {
                    timestamp: chrono::Utc::now(),
                    content: LogLineContent::ServerProcess(message.to_string()),
                });
            }
        };

        // set "pending" state
        log_info("Starting server...");
        self.server_states_tx.send_modify(|server_states| {
            server_states.push(ServerState {
                spec: spec.clone(),
                status: ServerStatus::Starting,
                logs_id: logs_id.clone(),
            });
        });

        // start server process
        let client = authenticate_filen_client(
            spec.filen_email.clone(),
            &spec.filen_password,
            spec.filen_2fa_code.clone(),
        )
        .await
        .context("Failed to authenticate Filen client using previously entered credentials")?;
        let config_dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("rclone_configs");
        let port = free_local_ipv4_port().context("Failed to find free local port")?;
        let mut server = filen_rclone_wrapper::serve::start_basic_server(
            &client,
            &RcloneInstallationConfig {
                rclone_binary_dir: config_dir.clone(),
                config_dir: config_dir.join(format!("server_{}", spec.id)),
            },
            match spec.server_type {
                ServerType::Http => "http",
                ServerType::Webdav => "webdav",
                ServerType::S3 => "s3",
                ServerType::Ftp => "ftp",
                ServerType::Sftp => "sftp",
            },
            BasicServerOptions {
                address: format!(":{}", port),
                root: Some(spec.root.clone()),
                user: None,
                password: spec.password.clone(),
                read_only: spec.read_only,
                cache_size: None,
                transfers: None,
            },
            vec![],
        )
        .await
        .context("Failed to start rclone server")?;

        // set "running" state
        log_info("Server started successfully.");
        self.server_states_tx.send_modify(|server_states| {
            if let Some(s) = server_states.iter_mut().find(|s| s.spec.id == spec.id) {
                s.status = ServerStatus::Running { port };
            }
        });

        let spec = spec.clone();

        // handle logs
        {
            let log_output = log_output.clone();
            let process_stdout = server.process.stdout.take().unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(process_stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log_output(&line);
                }
            });
        }
        {
            let process_stderr = server.process.stderr.take().unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(process_stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log_output(&line);
                }
            });
        }

        let (stop_server_tx, stop_server_rx) = oneshot::channel::<()>();
        self.stop_handles.insert(spec.id.clone(), stop_server_tx);
        let server_states_tx = self.server_states_tx.clone();
        tokio::spawn(async move {
            select! {
                _ = stop_server_rx => {
                    // handle stopping the server
                    if let Err(e) = server.process.kill().await {
                        log_err(&format!("Failed to stop server: {}", e));
                    } else {
                        log_info("Server stopped.");
                    }
                    server_states_tx.send_modify(|server_states| {
                        server_states.retain(|s| s.spec.id != spec.id);
                    });
                }
                status = server.process.wait() => {
                    // handle process exit
                    match status {
                        Ok(status) => {
                            log_err(&format!("Server process exited with status: {}", status));
                            if status.success() {
                                server_states_tx.send_modify(|server_states| {
                                    server_states.retain(|s| s.spec.id != spec.id);
                                });
                            } else {
                                server_states_tx.send_modify(|server_states| {
                                    if let Some(s) = server_states.iter_mut().find(|s| s.spec.id == spec.id)
                                    {
                                        s.status = ServerStatus::Error;
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            log_err(&format!("Server process wait failed: {}", e));
                            server_states_tx.send_modify(|server_states| {
                                if let Some(s) = server_states.iter_mut().find(|s| s.spec.id == spec.id) {
                                    s.status = ServerStatus::Error;
                                }
                            });
                        }
                    };
                }
            }
        });

        Ok(())
    }

    async fn stop_server(&mut self, spec: &ServerSpec) -> Result<()> {
        // send stop process
        let _ = self
            .stop_handles
            .remove(&spec.id)
            .ok_or_else(|| anyhow::anyhow!("No running server found with id: {} to stop", spec.id))?
            .send(()); // ignore failure, means the server is already stopped
        Ok(())
    }
    // todo: at some point also delete the directory?
}

impl ServerManagerApi {
    /// Returns a receiver to listen for server state updates.
    pub(crate) fn get_server_states(&self) -> tokio::sync::watch::Receiver<Vec<ServerState>> {
        self.server_states_rx.clone()
    }

    /// Returns a receiver to listen for logs.
    pub(crate) fn get_logs(&self, logs_id: &str) -> Option<Logs> {
        // todo: handle errors safely?
        let logs = self.logs.lock().unwrap();
        logs.get(logs_id).cloned()
    }

    /// Add/remove the server spec via the manager (will start/stop it) and persist it to the database.
    pub(crate) async fn update_server_spec(&self, update: ServerSpecUpdate) -> Result<()> {
        self.updates_tx
            .send(update)
            .await
            .context("Failed to send server spec update")
    }
}
