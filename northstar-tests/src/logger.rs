use anyhow::{anyhow, Context, Result};
use env_logger::Target;
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use std::{
    fmt,
    io::Write,
    time::{Duration, Instant},
};
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    time,
};

lazy_static! {
    /// Test started timestamp
    static ref START: Instant = Instant::now();
    /// Channel for log assumptions
    static ref QUEUE: (UnboundedSender<String>, Mutex<UnboundedReceiver<String>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, tokio::sync::Mutex::new(rx))
    };
}

/// Initialize logger
pub fn init() {
    lazy_static::initialize(&START);
    lazy_static::initialize(&QUEUE);

    env_logger::Builder::new()
        .parse_filters("debug")
        .target(Target::Stdout)
        .format(|buf, record| {
            let elapsed = START.elapsed();
            let timestamp = format!("{}.{:06}s", elapsed.as_secs(), elapsed.subsec_micros());
            let level = buf.default_styled_level(record.metadata().level());

            let tx = &QUEUE.0;
            tx.send(record.args().to_string()).expect("channel error");

            if let Some(module_path) = record
                .module_path()
                .and_then(|module_path| module_path.find(&"::").map(|p| &module_path[p + 2..]))
            {
                writeln!(
                    buf,
                    "{}: {:<5}: {} {}",
                    timestamp,
                    level,
                    module_path,
                    record.args(),
                )
            } else {
                writeln!(buf, "{}: {:<5}: {}", timestamp, level, record.args(),)
            }
        })
        .init()
}

/// Assume the runtime to log a line matching `pattern` within `timeout` seconds.
pub async fn assume<T: ToString + fmt::Display>(pattern: T, timeout: u64) -> Result<()> {
    time::timeout(Duration::from_secs(timeout), async {
        let regex = Regex::new(&pattern.to_string()).context("invalid regex")?;
        let mut rx = QUEUE.1.lock().await;
        loop {
            let n = rx.recv().await.ok_or_else(|| anyhow!("internal error"))?;
            if regex.is_match(&n) {
                debug!("Log assumption \"{}\" success", pattern);
                break Ok(());
            }
        }
    })
    .await
    .with_context(|| format!("timeout waiting for \"{}\"", pattern))?
}
