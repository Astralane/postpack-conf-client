use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agave_transaction_view::transaction_view::SanitizedTransactionView;
use bincode::Options;
use bytes::Bytes;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;

use crate::error::Error;
use crate::proto::PreconfStreamMessage;
use crate::proto::preconf_stream_message::Message;

/// A serialized transaction never exceeds one packet; anything larger is a malformed
/// length prefix.
const MAX_TRANSACTION_BYTES: u64 = 1232;

/// One byte of count, then the signature: no transaction has enough signatures for the
/// count to need a second byte.
const FIRST_SIGNATURE: std::ops::Range<usize> = 1..65;

/// A post-pack feed sends [`Event::Preconf`] only: it carries transactions the scheduler
/// committed to.
#[derive(Debug, Clone)]
pub enum Event {
    SlotStart(SlotStart),
    Preconf(Preconf),
    /// The connection was re-established. Messages may have been missed, and the
    /// current slot may repeat.
    StreamReset,
}

#[derive(Debug, Clone)]
pub struct SlotStart {
    pub slot: u64,
    pub leader: Pubkey,
    /// The leader's own clock, not synchronised with yours. `None` when that leader
    /// announces no slot boundary and the relay derived this one from the first
    /// transaction of the slot.
    pub timestamp: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct Preconf {
    pub slot: u64,
    /// The raw signed transaction, exactly as the leader executed it.
    pub data: Bytes,
}

impl Preconf {
    /// Read straight out of the payload, no deserialization. The relay does not
    /// forward transactions it failed to parse, so this does not fail in practice.
    pub fn signature(&self) -> Signature {
        self.data
            .get(FIRST_SIGNATURE)
            .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
            .map_or_else(Signature::default, Signature::from)
    }

    /// Zero copy access to accounts and instructions, borrowed from `data`.
    pub fn view(&self) -> Result<SanitizedTransactionView<&[u8]>, Error> {
        SanitizedTransactionView::try_new_sanitized(self.data.as_ref(), true)
            .map_err(|_| Error::Decode)
    }

    pub fn transaction(&self) -> Result<VersionedTransaction, Error> {
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .with_limit(MAX_TRANSACTION_BYTES)
            .deserialize(self.data.as_ref())
            .map_err(|_| Error::Decode)
    }
}

impl Event {
    /// `None` for a message this client cannot represent; the caller skips it.
    pub(crate) fn from_message(message: PreconfStreamMessage) -> Option<Self> {
        match message.message? {
            Message::ValidatorSlotStart(start) => {
                Some(Self::SlotStart(SlotStart {
                    slot: start.slot,
                    leader: Pubkey::try_from(start.leader_address.as_ref()).ok()?,
                    // Zero is how the relay reports a boundary it synthesised.
                    timestamp: (start.server_side_timestamp != 0)
                        .then(|| UNIX_EPOCH + Duration::from_nanos(start.server_side_timestamp)),
                }))
            }

            Message::Preconf(preconf) => Some(Self::Preconf(Preconf {
                slot: preconf.slot,
                data: preconf.data,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{self, ValidatorSlotStart};

    fn preconf(data: Vec<u8>) -> Preconf {
        Preconf {
            slot: 1,
            data: Bytes::from(data),
        }
    }

    #[test]
    fn a_preconf_keeps_the_slot_it_arrived_with() {
        let message = PreconfStreamMessage {
            message: Some(Message::Preconf(proto::Preconf {
                slot: 9,
                data: Bytes::from_static(b"transaction"),
            })),
        };
        match Event::from_message(message) {
            Some(Event::Preconf(preconf)) => assert_eq!(preconf.slot, 9),
            _ => panic!("expected a preconf"),
        }
    }

    #[test]
    fn signature_comes_from_the_payload_and_survives_a_truncated_one() {
        let mut data = vec![1];
        data.extend([7u8; 64]);
        data.extend([9u8; 32]);
        assert_eq!(preconf(data).signature(), Signature::from([7u8; 64]));

        assert_eq!(preconf(vec![1, 2, 3]).signature(), Signature::default());
    }

    #[test]
    fn a_zero_timestamp_reads_as_no_leader_clock() {
        let slot_start = |timestamp| PreconfStreamMessage {
            message: Some(Message::ValidatorSlotStart(ValidatorSlotStart {
                leader_address: Bytes::copy_from_slice(&[7u8; 32]),
                slot: 4,
                server_side_timestamp: timestamp,
            })),
        };
        let timestamp = |message| match Event::from_message(message) {
            Some(Event::SlotStart(start)) => start.timestamp,
            _ => panic!("expected a slot start"),
        };

        assert!(timestamp(slot_start(0)).is_none());
        assert!(timestamp(slot_start(1_700_000_000_000_000_000)).is_some());
    }

    #[test]
    fn slot_start_with_an_unusable_leader_is_skipped() {
        let message = PreconfStreamMessage {
            message: Some(Message::ValidatorSlotStart(ValidatorSlotStart {
                leader_address: Bytes::from_static(&[1, 2, 3]),
                slot: 4,
                server_side_timestamp: 5,
            })),
        };
        assert!(Event::from_message(message).is_none());

        let empty = PreconfStreamMessage { message: None };
        assert!(Event::from_message(empty).is_none());
    }
}
