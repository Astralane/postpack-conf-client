mod client;
mod config;
mod error;
mod event;
mod interests;

mod proto {
    tonic::include_proto!("preconf");
}

pub use client::{Client, PreconfStream};
pub use config::Config;
pub use error::Error;
pub use event::{Event, Preconf, SlotStart};
pub use interests::Interests;

// Re-exported so callers do not have to match Solana crate versions
pub use agave_transaction_view::transaction_view::SanitizedTransactionView;
pub use bytes::Bytes;
pub use solana_pubkey::{Pubkey, pubkey};
pub use solana_signature::Signature;
pub use solana_transaction::versioned::VersionedTransaction;
