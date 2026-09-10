use bytes::Bytes;
use solana_pubkey::Pubkey;

use crate::proto::SubscribePreconfsRequest;

/// Filters for what the relay sends. Both lists empty means everything, except on a
/// post-pack relay, which forwards only what matches and so then sends nothing.
#[derive(Debug, Clone, Default)]
pub struct Interests {
    accounts: Vec<Pubkey>,
    programs: Vec<Pubkey>,
}

impl Interests {
    /// Everything the relay is willing to send you, which from a post-pack relay is nothing.
    pub fn all() -> Self {
        Self::default()
    }

    /// An empty builder, which is [`all`](Self::all) until something is added to it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds accounts, matched only when a transaction can write them. A program id here
    /// matches almost nothing; pass it to [`programs`](Self::programs) instead.
    pub fn accounts(mut self, keys: impl IntoIterator<Item = Pubkey>) -> Self {
        self.accounts.extend(keys);
        self
    }

    /// Adds programs, matched in any account position, writable or not.
    pub fn programs(mut self, keys: impl IntoIterator<Item = Pubkey>) -> Self {
        self.programs.extend(keys);
        self
    }

    pub(crate) fn request(&self) -> SubscribePreconfsRequest {
        SubscribePreconfsRequest {
            accounts_of_interest: encode(&self.accounts),
            programs_of_interest: encode(&self.programs),
        }
    }
}

fn encode(keys: &[Pubkey]) -> Vec<Bytes> {
    keys.iter()
        .map(|key| Bytes::copy_from_slice(key.as_ref()))
        .collect()
}
