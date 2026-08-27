use std::time::Duration;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) endpoint: String,
    pub(crate) token: String,
    pub(crate) connect_timeout: Duration,
}

impl Config {
    /// `endpoint` decides the transport: `https://` uses the system root certificates,
    /// `http://` connects in the clear. `token` is your base58 public key.
    pub fn new(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            token: token.into(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }

    /// Bounds establishing the connection only, never the subscription stream itself.
    /// Defaults to 10 seconds.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }
}
