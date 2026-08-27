# astralane-preconf-client

Subscriber for the Astralane preconfirmation relay: transactions streamed by the leader that
scheduled them, before the block holding them exists.

```rust
use astralane_preconf_client::{Client, Config, Event, Interests};

let mut client = Client::connect(Config::new(endpoint, token)).await?;
let mut stream = client.subscribe(Interests::all()).await?;

while let Some(event) = stream.next().await? {
    match event {
        Event::SlotStart(start) => println!("slot {} opened by {}", start.slot, start.leader),
        Event::Preconf(tx) => println!("{} index={}", tx.signature(), tx.index),
        Event::StreamReset => eprintln!("reconnected, there may be a gap"),
    }
}
```

The crate handles TLS, the `x-token` header, reconnects with resubscription, and transaction
decoding. What it deliberately does not do is hide a gap: a reconnect is reported as
`Event::StreamReset`, because the stream may have skipped messages and may repeat the current slot.

Run it against a relay:

```sh
PRECONF_TOKEN=<your base58 public key> cargo run --example subscribe
```

Access is granted per public key. What the stream contains, how filtering works and what the
errors mean are documented in the preconfirmations guide.