# astralane-preconf-client

Subscriber for Astralane's preconfirmation and post-pack relays: transactions streamed by the
leader that scheduled them, before the block holding them exists.

```rust
use astralane_preconf_client::{Client, Config, Event, Interests};

let mut client = Client::connect(Config::new(endpoint, token)).await?;
let mut stream = client.subscribe(Interests::all()).await?;

while let Some(event) = stream.next().await? {
    match event {
        Event::SlotStart(start) => println!("slot {} opened by {}", start.slot, start.leader),
        Event::Preconf(tx) => println!("{}", tx.signature()),
        Event::StreamReset => eprintln!("reconnected, there may be a gap"),
    }
}
```

The crate handles TLS, the `x-token` header, reconnects with resubscription, and transaction
decoding. What it deliberately does not do is hide a gap: a reconnect is reported as
`Event::StreamReset`, because the stream may have skipped messages and may repeat the current slot.

A **post-pack** feed is the same schema with less in it: transactions the leader's scheduler
committed to, and nothing else. No slot boundaries arrive. `slot` is the slot the block engine
assigned the transaction to where it reports one, and otherwise the relay's own view of the
current slot, which can trail the leader by a slot or two.
Each transaction carries `leader`, the validator that produced it, when the relay knows it.

Interests filter it like any other feed, and naming no accounts or programs gets you everything.
Filter kinds combine with AND, keys within one list with OR: a transaction has to touch an
account or program of interest (when any are named), be signed by one of `signers` (when any are
named), and be signed by none of `excluded_signers` and reference none of `excluded_accounts`.

Run it against a relay:

```sh
PRECONF_TOKEN=<your access token> cargo run --example subscribe
```

Or keep the settings in a `.env` file: copy `.env.example` to `.env` and fill it in. Besides the
endpoint and token it takes `PRECONF_ACCOUNTS`, `PRECONF_PROGRAMS`, `PRECONF_EXCLUDED_ACCOUNTS`,
`PRECONF_SIGNERS` and `PRECONF_EXCLUDED_SIGNERS`, comma separated pubkeys to filter on.

The example is meant to be left running. Every 15 seconds it logs how many transactions arrived,
to the console and to a daily file under `logs/` (`PRECONF_LOG_DIR`). `RUST_LOG=debug` logs each
transaction as well.

Access is granted per token. What the stream contains, how filtering works and what the errors
mean are documented in the preconfirmations guide.