use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use monoio::spawn;
use monolake_core::{
    config::ServiceConfig,
    orchestrator::{ServiceCommand, WorkerManager},
};
use service_async::AsyncMakeService;

use crate::config::{Config, ListenerConfig, ServerConfig};

type ServiceConfigMap = HashMap<String, ServiceConfig<ListenerConfig, ServerConfig>>;

pub struct StaticFileConfigManager<F, LF, FP, LFP>
where
    FP: Fn(ServerConfig) -> F,
    LFP: Fn(ListenerConfig) -> LF,
{
    online_config_content: RefCell<Vec<u8>>,
    online_services: RefCell<ServiceConfigMap>,
    worker_manager: WorkerManager<F, LF>,
    listener_factory_provider: LFP,
    server_factory_provider: FP,
}

impl<F, LF, FP, LFP> StaticFileConfigManager<F, LF, FP, LFP>
where
    F: Send + Clone + 'static,
    LF: Send + Clone + 'static,
    FP: 'static,
    LFP: 'static,
    F: AsyncMakeService,
    FP: Fn(ServerConfig) -> F,
    LFP: Fn(ListenerConfig) -> LF,
{
    pub fn new(
        worker_manager: WorkerManager<F, LF>,
        listener_factory_provider: LFP,
        server_factory_provider: FP,
    ) -> Self {
        Self {
            online_config_content: Default::default(),
            online_services: Default::default(),
            worker_manager,
            listener_factory_provider,
            server_factory_provider,
        }
    }

    pub async fn load_and_watch(mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.reload_file(&path).await?;
        self.watch(path.as_ref().to_path_buf()).await;
        Ok(())
    }

    async fn reload_file(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let latest_content = monolake_core::util::file_read(path).await?;
        if self.online_config_content.borrow().eq(&latest_content) {
            return Ok(());
        }

        tracing::info!("config change detected, reloading");
        let new_services = Config::parse_service_config(&latest_content)?;
        self.reload_services(&new_services).await?;

        tracing::info!("config reload success");
        self.online_config_content.replace(latest_content);
        self.online_services.replace(new_services);
        Ok(())
    }

    async fn reload_services(&mut self, new_services: &ServiceConfigMap) -> anyhow::Result<()> {
        let patches = Self::diff(&self.online_services.borrow(), new_services);
        match self.prepare(&patches).await {
            Ok(_) => {
                self.commit(&patches)
                    .await
                    .expect("config reload failed at commit stage");
                Ok(())
            }
            Err(e) => {
                tracing::error!("config reload failed at prepare stage: {}, aborting", e);
                self.abort(&patches)
                    .await
                    .expect("abort config reload failed");
                Err(e)
            }
        }
    }

    fn diff(old_services: &ServiceConfigMap, new_services: &ServiceConfigMap) -> Vec<Patch> {
        let mut patches = Vec::new();

        let old_keys = old_services.keys().collect::<HashSet<_>>();
        let new_keys = new_services.keys().collect::<HashSet<_>>();
        let all_keys = old_keys.union(&new_keys).collect::<HashSet<_>>();
        for key in all_keys {
            let patch = match (old_keys.contains(key), new_keys.contains(key)) {
                (true, true) => {
                    // TODO: Skip keys whose configuration didn't change
                    let new_config = new_services.get(*key).unwrap();
                    Patch::Update {
                        key: key.to_string(),
                        server_config: new_config.server.clone(),
                    }
                }
                (true, false) => Patch::Delete {
                    key: key.to_string(),
                },
                (false, true) => {
                    let new_config = new_services.get(*key).unwrap();
                    Patch::Insert {
                        key: key.to_string(),
                        listener_config: new_config.listener.clone(),
                        server_config: new_config.server.clone(),
                    }
                }
                (false, false) => {
                    panic!("unexpected error: illegal key {key}");
                }
            };
            patches.push(patch);
        }
        patches
    }

    async fn prepare(&mut self, patches: &[Patch]) -> anyhow::Result<()> {
        for patch in patches {
            match patch {
                Patch::Insert {
                    key, server_config, ..
                }
                | Patch::Update {
                    key, server_config, ..
                } => {
                    self.worker_manager
                        .dispatch_service_command(ServiceCommand::Precommit(
                            Arc::new(key.to_string()),
                            (self.server_factory_provider)(server_config.clone()),
                        ))
                        .await
                        .err()?;
                }
                Patch::Delete { .. } => {
                    // nothing to do at prepare stage
                }
            }
        }
        Ok(())
    }

    async fn commit(&mut self, patches: &[Patch]) -> anyhow::Result<()> {
        for patch in patches {
            match patch {
                Patch::Insert {
                    key,
                    listener_config,
                    ..
                } => {
                    self.worker_manager
                        .dispatch_service_command(ServiceCommand::Commit(
                            Arc::new(key.to_string()),
                            (self.listener_factory_provider)(listener_config.clone()),
                        ))
                        .await
                        .err()?;
                }
                Patch::Update { key, .. } => {
                    self.worker_manager
                        .dispatch_service_command(ServiceCommand::Update(Arc::new(key.to_string())))
                        .await
                        .err()?;
                }
                Patch::Delete { key } => {
                    self.worker_manager
                        .dispatch_service_command(ServiceCommand::Remove(Arc::new(key.to_string())))
                        .await
                        .err()?;
                }
            }
        }
        Ok(())
    }

    async fn abort(&mut self, patches: &[Patch]) -> anyhow::Result<()> {
        for patch in patches {
            match patch {
                Patch::Insert { key, .. } | Patch::Update { key, .. } => {
                    self.worker_manager
                        .dispatch_service_command(ServiceCommand::Abort(Arc::new(key.to_string())))
                        .await; // discard errors due to partial pre-commits
                }
                Patch::Delete { .. } => {
                    // nothing to do at abort stage
                }
            }
        }
        Ok(())
    }

    async fn watch(mut self, path: PathBuf) {
        spawn(async move {
            loop {
                if let Err(e) = self.reload_file(&path).await {
                    tracing::error!("reload config failed: {}", e);
                }
                monoio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await;
    }
}

enum Patch {
    Insert {
        key: String,
        listener_config: ListenerConfig,
        server_config: ServerConfig,
    },
    Update {
        key: String,
        server_config: ServerConfig, // ListenerConfig dynamic update not supported yet
    },
    Delete {
        key: String,
    },
}
