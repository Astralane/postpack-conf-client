//! Prints the transactions as they arrive, and slot boundaries if the feed sends them.
//!
//! ```sh
//! PRECONF_TOKEN=<your access token> cargo run -p astralane-preconf-client --example subscribe
//! ```

use astralane_preconf_client::{Client, Config, Event, Interests};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:12349";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint =
        std::env::var("PRECONF_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
    let token = std::env::var("PRECONF_TOKEN")
        .map_err(|_| anyhow::anyhow!("set PRECONF_TOKEN to your access token"))?;

    let mut client = Client::connect(Config::new(endpoint, token)).await?;
    let mut stream = client.subscribe(Interests::all()).await?;

    while let Some(event) = stream.next().await? {
        match event {
            Event::SlotStart(start) => println!("slot {} opened by {}", start.slot, start.leader),
            Event::Preconf(tx) => println!("  {}", tx.signature()),
            Event::StreamReset => eprintln!("reconnected, some messages may have been missed"),
        }
    }
    Ok(())
}
