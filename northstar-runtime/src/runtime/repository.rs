use super::{
    key::{self, PublicKey},
    Container,
};
use crate::{npk::npk::Npk as NpkNpk, runtime::ipc::RawFdExt};
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures::{future::try_join_all, FutureExt};
use log::{debug, info, warn};
use mpsc::Receiver;
use nanoid::nanoid;
use std::{
    collections::HashMap,
    fmt,
    future::ready,
    io::{BufReader, SeekFrom},
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd},
    path::{Path, PathBuf},
};
use tokio::{
    fs::{self},
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::mpsc,
    task,
    time::Instant,
};

pub(super) type Npk = NpkNpk<BufReader<std::fs::File>>;

#[async_trait::async_trait]
pub(super) trait Repository: fmt::Debug {
    /// Stream an npk from `rx` into the repository and load it
    async fn insert(&mut self, rx: &mut Receiver<Bytes>) -> Result<Container>;

    /// Add container from repository if present
    async fn remove(&mut self, container: &Container) -> Result<()>;

    /// Return npk matching container if present
    fn get(&self, container: &Container) -> Option<&Npk>;

    /// Key of this repository
    fn key(&self) -> Option<&PublicKey>;

    /// All containers in this repository
    fn containers(&self) -> Vec<&Npk>;
}

/// Repository backed by a directory
#[derive(Debug)]
pub(super) struct DirRepository {
    dir: PathBuf,
    key: Option<PublicKey>,
    containers: HashMap<Container, (PathBuf, Npk)>,
}

impl DirRepository {
    pub async fn new(dir: &Path, key: Option<&Path>) -> Result<DirRepository> {
        let mut containers = HashMap::new();

        // Load key
        let key = if let Some(key) = key {
            info!(
                "Loading repository {} with key {}",
                dir.display(),
                dir.display()
            );
            Some(key::load(key).await.context("failed to load key")?)
        } else {
            info!("Loading repository {} (unverified)", dir.display());
            None
        };

        let mut readir = fs::read_dir(&dir)
            .await
            .with_context(|| format!("failed to read dir {}", dir.display()))?;

        let start = Instant::now();
        let mut tasks = Vec::new();
        while let Ok(Some(entry)) = readir.next_entry().await {
            let file = entry.path();
            let load_task = task::spawn_blocking(move || {
                debug!(
                    "Loading {}{}",
                    file.display(),
                    if key.is_some() { " [verified]" } else { "" }
                );
                let reader = std::fs::File::open(&file)
                    .with_context(|| format!("failed to open {}", file.display()))?;
                let reader = std::io::BufReader::new(reader);
                let npk = NpkNpk::from_reader(reader, key.as_ref())
                    .with_context(|| format!("failed to read npk {}", file.display()))?;
                let name = npk.manifest().name.clone();
                let version = npk.manifest().version.clone();
                let container = Container::new(name, version);
                Result::<_, anyhow::Error>::Ok((container, (file, npk)))
            })
            .then(|r| ready(r.expect("Task error")));

            tasks.push(load_task);
        }

        for result in try_join_all(tasks).await? {
            let (container, (file, npk)) = result;
            containers.insert(container, (file, npk));
        }

        let duration = start.elapsed();
        info!(
            "Loaded {} containers from {} in {:.03}s",
            containers.len(),
            dir.display(),
            duration.as_secs_f32(),
        );

        Ok(DirRepository {
            dir: dir.to_owned(),
            key,
            containers,
        })
    }
}

#[async_trait::async_trait]
impl Repository for DirRepository {
    async fn insert(&mut self, rx: &mut Receiver<Bytes>) -> Result<Container> {
        let dest = self.dir.join(format!("{}.npk", nanoid!()));
        let mut file = fs::File::create(&dest)
            .await
            .with_context(|| format!("failed create repository {}", dest.display()))?;
        while let Some(r) = rx.recv().await {
            file.write_all(&r).await.context("failed to write npk")?;
        }
        file.flush().await.context("failed to flush npk")?;
        drop(file);

        debug!("Loading temporary npk {}", dest.display());
        let npk = match Npk::from_path(dest.as_path(), self.key.as_ref())
            .with_context(|| format!("failed to read npk {}", dest.display()))
        {
            Ok(n) => Ok(n),
            Err(e) => {
                fs::remove_file(&dest)
                    .await
                    .with_context(|| format!("failed to remove {}", dest.display()))?;
                Err(e)
            }
        }?;
        let container = npk.manifest().container();
        info!("Loaded {} from {}", container, dest.display());

        // Check of the container is present
        if self.containers.contains_key(&container) {
            warn!("Container {} is already present in repository", container);
            fs::remove_file(&dest)
                .await
                .with_context(|| format!("failed to remove {}", dest.display()))?;
            bail!("{} already in {}", container, self.dir.display())
        } else {
            let old = dest;
            let new = self.dir.join(format!("{}.npk", container));
            debug!("Moving {} to {}", old.display(), new.display());
            // Renaming a file with an open fd is ok if the file remains on the same fs.
            // The rename here is in the same directory, so it's ok.
            fs::rename(&old, &new)
                .await
                .context("Rename file in repository")?;
            self.containers.insert(container.clone(), (new, npk));
            Ok(container)
        }
    }

    async fn remove(&mut self, container: &Container) -> Result<()> {
        let (path, npk) = self
            .containers
            .remove(container)
            .expect("Container not found");
        debug!("Removing {}", path.display());
        drop(npk);
        fs::remove_file(&path)
            .await
            .with_context(|| format!("failed to remove {}", path.display()))?;
        Ok(())
    }

    fn get(&self, container: &Container) -> Option<&Npk> {
        self.containers.get(container).map(|(_, npk)| npk)
    }

    fn key(&self) -> Option<&PublicKey> {
        self.key.as_ref()
    }

    fn containers(&self) -> Vec<&Npk> {
        self.containers.values().map(|(_, npk)| npk).collect()
    }
}

/// In memory repository
#[derive(Debug)]
pub(super) struct MemRepository {
    key: Option<PublicKey>,
    containers: HashMap<Container, Npk>,
}

impl MemRepository {
    pub async fn new(key: Option<&Path>) -> Result<MemRepository> {
        let key = if let Some(key) = key {
            info!("Loading memory repository with key {}", key.display());
            Some(key::load(key).await.context("failed to load key")?)
        } else {
            info!("Loading repository (unverified)");
            None
        };

        Ok(MemRepository {
            key,
            containers: HashMap::new(),
        })
    }
}

#[async_trait::async_trait]
impl Repository for MemRepository {
    async fn insert(&mut self, rx: &mut Receiver<Bytes>) -> Result<Container> {
        // Create a new memfd
        let opts = memfd::MemfdOptions::default().allow_sealing(true);
        let fd = opts.create(nanoid!()).context("failed to create memfd")?;

        // Write buffer to the memfd
        let mut file = unsafe { fs::File::from_raw_fd(fd.as_raw_fd()) };
        file.set_nonblocking(true)
            .context("failed to set nonblocking")?;

        while let Some(r) = rx.recv().await {
            file.write_all(&r).await.context("failed stream npk")?;
        }

        file.seek(SeekFrom::Start(0)).await.context("failed seek")?;

        // Seal the memfd
        let seals = memfd::SealsHashSet::from_iter([
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealWrite,
        ]);
        fd.add_seals(&seals)
            .and_then(|_| fd.add_seal(memfd::FileSeal::SealSeal))
            .context("failed to add memfd seals")?;

        // Forget fd - it's owned by file
        fd.into_raw_fd();

        file.set_nonblocking(false)
            .context("failed to set blocking")?;
        let file = BufReader::new(file.into_std().await);

        // Load npk
        debug!("Loading memfd as npk");
        let npk = NpkNpk::from_reader(file, self.key.as_ref()).context("failed to read npk")?;
        let container = npk.manifest().container();
        info!("Loaded {} from memfd", container);

        if self.containers.contains_key(&container) {
            warn!(
                "Container {} is already present in repository. Dropping...",
                container
            );
            bail!("{} already in repository", container)
        } else {
            self.containers.insert(container.clone(), npk);
            Ok(container)
        }
    }

    async fn remove(&mut self, container: &Container) -> Result<()> {
        debug_assert!(self.containers.contains_key(container));
        self.containers.remove(container);
        Ok(())
    }

    fn get(&self, container: &Container) -> Option<&Npk> {
        self.containers.get(container)
    }

    fn containers(&self) -> Vec<&Npk> {
        self.containers.values().collect()
    }

    fn key(&self) -> Option<&PublicKey> {
        self.key.as_ref()
    }
}
