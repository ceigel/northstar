use super::{
    config::{debug, Config},
    error::Error,
    Pid,
};
use crate::npk::manifest::Manifest;
use anyhow::Context;
use futures::future::OptionFuture;
use log::{debug, error, info};
use std::{path::Path, process::Stdio};
use tokio::{
    fs,
    io::{self, AsyncBufReadExt},
    process::{Child, Command},
    select,
    task::{self, JoinHandle},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct Strace {
    child: Child,
    token: CancellationToken,
    task: JoinHandle<()>,
}

/// Debugging facilities attached to a started container
#[derive(Debug)]
pub(crate) struct Debug {
    strace: Option<Strace>,
    perf: Option<Perf>,
}

impl Debug {
    /// Start configured debug facilities and attach to `pid`
    pub(crate) async fn new(
        config: &Config,
        manifest: &Manifest,
        pid: Pid,
    ) -> Result<Debug, Error> {
        // Attach a strace instance if configured in the runtime configuration
        let strace: OptionFuture<_> = config
            .debug
            .as_ref()
            .and_then(|debug| debug.strace.as_ref())
            .map(|strace| Strace::new(strace, manifest, &config.log_dir, pid))
            .into();

        // Attach a perf instance if configured in the runtime configuration
        let perf: OptionFuture<_> = config
            .debug
            .as_ref()
            .and_then(|debug| debug.perf.as_ref())
            .map(|perf| Perf::new(perf, manifest, &config.log_dir, pid))
            .into();

        let (strace, perf) = tokio::join!(strace, perf);
        Ok(Debug {
            strace: strace.transpose()?,
            perf: perf.transpose()?,
        })
    }

    /// Shutdown configured debug facilities and attached to `pid`
    pub(crate) async fn destroy(mut self) -> Result<(), super::error::Error> {
        if let Some(strace) = self.strace.take() {
            strace.destroy().await?;
        }

        if let Some(perf) = self.perf.take() {
            perf.destroy().await?;
        }

        Ok(())
    }
}

impl Strace {
    /// Create a new Strace instance by starting strace and attaching it to the pid
    /// of the started application. Forward the stderror of strace to the configured sink.
    pub async fn new(
        strace: &debug::Strace,
        manifest: &Manifest,
        log_dir: &Path,
        pid: Pid,
    ) -> Result<Strace, Error> {
        let cmd = if let Some(ref strace) = strace.path {
            strace.as_path()
        } else {
            Path::new("strace")
        };
        let mut child = Command::new(cmd)
            .arg("-p")
            .arg(pid.to_string())
            .args(
                strace
                    .flags
                    .as_ref()
                    .cloned()
                    .unwrap_or_default()
                    .split_whitespace(),
            )
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn strace")?;
        debug!("Attached strace to PID {}", &pid.to_string());

        let token = CancellationToken::new();
        let stderr = child.stderr.take().expect("failed to get stderr of strace");

        // Wait for strace to inform us that it's attached.
        let mut stderr = io::BufReader::new(stderr).lines();
        stderr.next_line().await.context("reading strace stderr")?;

        let task = {
            let token = token.clone();
            let log_dir = log_dir.to_owned();
            let name = manifest.name.clone();
            let strace = strace.clone();

            task::spawn(async move {
                // Discard until execve if configured
                if !strace.include_runtime.unwrap_or_default() {
                    loop {
                        match stderr.next_line().await {
                            Ok(Some(l)) if l.contains("execve(") => break,
                            Ok(None) => return,
                            _ => continue,
                        }
                    }
                };

                match strace.output {
                    debug::StraceOutput::File => {
                        let mut stderr = stderr.into_inner();
                        let mut filename = log_dir.join(format!("strace-{}-{}.strace", pid, name));
                        let mut n = 0u32;
                        while filename.exists() {
                            n += 1;
                            let name = format!("strace-{}-{}-{}.strace", pid, name, n);
                            filename = log_dir.join(name);
                        }

                        info!("Dumping strace output to {}", filename.display());

                        let mut file = match fs::File::create(&filename).await {
                            Ok(file) => file,
                            Err(e) => {
                                error!("failed to write strace output: {}", e);
                                return;
                            }
                        };

                        select! {
                            _ = token.cancelled() => (),
                            result = tokio::io::copy_buf(&mut stderr, &mut file) => {
                                if let Err(e) = result {
                                    error!("failed to write strace output: {}", e);
                                }
                            }
                        }
                    }
                    debug::StraceOutput::Log => loop {
                        select! {
                            _ = token.cancelled() => break,
                            stderr = stderr.next_line() => {
                                match stderr {
                                    Ok(Some(line)) => debug!("{}: {}", name, line),
                                    Ok(None) => break,
                                    Err(e) => {
                                        error!("failed to forward strace output: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    },
                }
            })
        };

        Ok(Strace { child, token, task })
    }

    pub async fn destroy(mut self) -> Result<(), Error> {
        // Stop the strace output forwarding
        self.token.cancel();
        self.task.await.context("Join error")?;

        // Stop strace - if not already existed - ignore any error
        let pid = self.child.id().expect("missing process id");
        self.child.kill().await.ok();
        debug!("Joining strace pid {}", pid);
        self.child.wait().await.context("failed to join strace")?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Perf {
    child: Child,
}

impl Perf {
    pub async fn new(
        perf: &debug::Perf,
        manifest: &Manifest,
        log_dir: &Path,
        pid: Pid,
    ) -> Result<Perf, Error> {
        let mut filename = log_dir.join(format!("perf-{}-{}.perf", pid, manifest.name));
        let mut n = 0u32;
        while filename.exists() {
            n += 1;
            filename = log_dir.join(format!("perf-{}-{}-{}.perf", pid, manifest.name, n));
        }

        info!("Dumping perf output to {}", filename.display());

        let cmd = if let Some(ref perf) = perf.path {
            perf.as_path()
        } else {
            Path::new("perf")
        };
        let child = Command::new(cmd)
            .arg("record")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg(filename.display().to_string())
            .args(
                perf.flags
                    .as_ref()
                    .cloned()
                    .unwrap_or_default()
                    .split_whitespace(),
            )
            .spawn()
            .context("failed to spawn strace")?;
        Ok(Perf { child })
    }

    pub async fn destroy(mut self) -> Result<(), Error> {
        let pid = self.child.id().expect("missing process id");
        self.child.kill().await.ok();
        debug!("Joining perf pid {}", pid);
        self.child.wait().await.context("failed to join perf")?;

        Ok(())
    }
}
