use tonic::{Code, Status};

/// A failure the client could not handle itself. Anything it retries internally never
/// reaches here.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connect to {endpoint}: {source}")]
    Connect {
        endpoint: String,
        source: tonic::transport::Error,
    },
    #[error("stream failed: {0}")]
    Transport(Status),
    /// The relay rejected the token. The stream ends here instead of reconnecting.
    #[error("rejected by the relay: {0}")]
    Auth(Status),
    /// The relay rejected the interest filters. The stream ends here instead of
    /// reconnecting; resubscribe with different ones.
    #[error("interest rejected by the relay: {0}")]
    InvalidInterest(Status),
    #[error("malformed transaction payload")]
    Decode,
}

impl Error {
    pub(crate) fn from_status(status: Status) -> Self {
        match status.code() {
            Code::Unauthenticated | Code::PermissionDenied => Self::Auth(status),
            Code::InvalidArgument => Self::InvalidInterest(status),
            _ => Self::Transport(status),
        }
    }

    /// True when the relay rejected the request itself, so a reconnect returns the same status
    #[inline]
    pub(crate) fn is_permanent(&self) -> bool {
        matches!(self, Self::Auth(_) | Self::InvalidInterest(_))
    }
}
