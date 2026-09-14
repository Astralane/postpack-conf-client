//! Keeps a subscription open and reports every 15 seconds how many transactions arrived.
//!
//! ```sh
//! PRECONF_TOKEN=<your access token> cargo run -p astralane-preconf-client --example subscribe
//! ```
//!
//! Settings can also live in a `.env` file in the working directory, see `.env.example`.
//! Variables already set in the environment win over the file.
//!
//! Logs go to the console and to a file under `PRECONF_LOG_DIR` (`logs` by default) that
//! rolls over daily. `RUST_LOG=debug` adds every transaction and slot boundary.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Context;
use astralane_preconf_client::{Client, Config, Event, Interests, Pubkey};
use tokio::time::{Instant, interval_at};
use tracing::{debug, error, info, warn};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:12349";
const DEFAULT_LOG_DIR: &str = "logs";
const REPORT_EVERY: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    // A missing .env is fine, a malformed one is not
    if let Err(error) = dotenvy::dotenv()
        && !error.not_found()
    {
        return Err(error).context("load .env");
    }
    init_logging()?;

    if let Err(error) = run().await {
        // Logged rather than returned, so it lands in the log file as well
        error!("{error:#}");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

async fn run() -> anyhow::Result<()> {
    let endpoint = var("PRECONF_ENDPOINT").unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
    let token = var("PRECONF_TOKEN").context("set PRECONF_TOKEN to your access token")?;
    let accounts = pubkeys("PRECONF_ACCOUNTS")?;
    let programs = pubkeys("PRECONF_PROGRAMS")?;
    info!(%endpoint, accounts = accounts.len(), programs = programs.len(), "connecting");

    // Started first so the reports double as a heartbeat, connected or not
    let received = Arc::new(AtomicU64::new(0));
    tokio::spawn(report(received.clone()));

    let mut client = Client::connect(Config::new(endpoint, token)).await?;
    let interests = Interests::new().accounts(accounts).programs(programs);
    let mut stream = client.subscribe(interests).await?;
    info!("subscribed");

    while let Some(event) = stream.next().await? {
        match event {
            Event::SlotStart(start) => {
                debug!(slot = start.slot, leader = %start.leader, "slot opened")
            }
            Event::Preconf(tx) => {
                received.fetch_add(1, Ordering::Relaxed);
                debug!(slot = tx.slot, signature = %tx.signature(), "transaction");
            }
            Event::StreamReset => warn!("reconnected, some messages may have been missed"),
        }
    }
    Ok(())
}

/// Every [`REPORT_EVERY`], how many transactions arrived since the previous report
async fn report(received: Arc<AtomicU64>) {
    let mut ticks = interval_at(Instant::now() + REPORT_EVERY, REPORT_EVERY);
    let mut total = 0;
    loop {
        ticks.tick().await;
        let count = received.swap(0, Ordering::Relaxed);
        total += count;
        info!(
            "{count} transactions in the last {}s, {total} since start",
            REPORT_EVERY.as_secs()
        );
    }
}

/// The console and a daily file under `PRECONF_LOG_DIR`, both timestamped in UTC.
/// `RUST_LOG` sets the level, info when unset.
fn init_logging() -> anyhow::Result<()> {
    let dir = var("PRECONF_LOG_DIR").unwrap_or_else(|| DEFAULT_LOG_DIR.to_string());
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("subscribe")
        .filename_suffix("log")
        .build(&dir)
        .with_context(|| format!("open a log file in {dir}"))?;
    tracing_subscriber::registry()
        .with(EnvFilter::new(var("RUST_LOG").as_deref().unwrap_or("info")))
        // Colors only for a person at a terminal, not when piped to a file or journald
        .with(fmt::layer().with_ansi(std::io::stdout().is_terminal()))
        .with(fmt::layer().with_ansi(false).with_writer(file))
        .init();
    Ok(())
}

/// Unset and empty are the same, so a `.env` can leave a line blank
fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Comma separated base58 keys
fn pubkeys(name: &str) -> anyhow::Result<Vec<Pubkey>> {
    var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| {
            key.parse::<Pubkey>()
                .with_context(|| format!("{name}: {key:?} is not a pubkey"))
        })
        .collect()
}
