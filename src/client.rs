use std::sync::Once;
use std::time::Duration;

use tokio::time::sleep;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Request, Status, Streaming};
use tracing::warn;

use crate::config::Config;
use crate::error::Error;
use crate::event::Event;
use crate::interests::Interests;
use crate::proto::preconf_service_client::PreconfServiceClient;
use crate::proto::{PreconfStreamMessage, SubscribePreconfsRequest};

const MIN_RECONNECT_BACKOFF: Duration = Duration::from_millis(250);
const MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(10);

pub struct Client {
    connection: Connection,
    channel: Channel,
}

impl Client {
    pub async fn connect(config: Config) -> Result<Self, Error> {
        install_crypto_provider();
        let connection = Connection::new(config)?;
        let channel = connection.channel().await?;
        Ok(Self {
            connection,
            channel,
        })
    }

    /// The interests are kept, so a reconnect resubscribes with the same ones.
    pub async fn subscribe(&mut self, interests: Interests) -> Result<PreconfStream, Error> {
        let request = interests.request();
        let stream = self
            .connection
            .subscribe(self.channel.clone(), &request)
            .await?;
        Ok(PreconfStream {
            connection: self.connection.clone(),
            request,
            stream: Some(stream),
            reset_pending: false,
            finished: false,
        })
    }
}

pub struct PreconfStream {
    connection: Connection,
    request: SubscribePreconfsRequest,
    stream: Option<Streaming<PreconfStreamMessage>>,
    reset_pending: bool,
    finished: bool,
}

impl PreconfStream {
    /// Broken connections are retried underneath and surface as [`Event::StreamReset`].
    /// An error means the relay refused the subscription; every later call then returns
    /// `Ok(None)` instead of reconnecting.
    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        loop {
            if self.finished {
                return Ok(None);
            }
            if self.reset_pending {
                self.reset_pending = false;
                return Ok(Some(Event::StreamReset));
            }

            let Some(stream) = self.stream.as_mut() else {
                self.reconnect().await?;
                continue;
            };

            match stream.message().await {
                Ok(Some(message)) => {
                    if let Some(event) = Event::from_message(message) {
                        return Ok(Some(event));
                    }
                }

                // An endless stream ending cleanly means the relay went away, which is
                // no different from the connection breaking
                Ok(None) => self.stream = None,
                Err(status) => {
                    let error = Error::from_status(status);
                    if error.is_permanent() {
                        self.finished = true;
                        return Err(error);
                    }
                    warn!(%error, "preconf stream broke");
                    self.stream = None;
                }
            }
        }
    }

    async fn reconnect(&mut self) -> Result<(), Error> {
        let mut backoff = MIN_RECONNECT_BACKOFF;
        loop {
            match self.connection.open(&self.request).await {
                Ok(stream) => {
                    self.stream = Some(stream);
                    self.reset_pending = true;
                    return Ok(());
                }
                Err(error) if error.is_permanent() => {
                    self.finished = true;
                    return Err(error);
                }
                Err(error) => {
                    warn!(%error, retry_in_ms = backoff.as_millis() as u64, "preconf reconnect failed");
                    sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_RECONNECT_BACKOFF);
                }
            }
        }
    }
}

/// What it takes to open the subscription, shared by the first attempt and the retries.
#[derive(Clone)]
struct Connection {
    url: String,
    endpoint: Endpoint,
    token: MetadataValue<Ascii>,
}

impl Connection {
    fn new(config: Config) -> Result<Self, Error> {
        let token = MetadataValue::try_from(config.token)
            .map_err(|_| Error::Auth(Status::unauthenticated("x-token must be printable ascii")))?;
        let url = config.endpoint;
        // No request timeout: the call is an open ended stream and a timeout would cut it.
        let mut endpoint = Endpoint::from_shared(url.clone())
            .map_err(|source| Error::Connect {
                endpoint: url.clone(),
                source,
            })?
            .connect_timeout(config.connect_timeout)
            // Without keepalive a dead connection looks idle and the stream hangs.
            .http2_keep_alive_interval(Duration::from_secs(10))
            .keep_alive_timeout(Duration::from_secs(5))
            .http2_adaptive_window(true);
        if url.starts_with("https://") {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map_err(|source| Error::Connect {
                    endpoint: url.clone(),
                    source,
                })?;
        }
        Ok(Self {
            url,
            endpoint,
            token,
        })
    }

    async fn channel(&self) -> Result<Channel, Error> {
        self.endpoint
            .connect()
            .await
            .map_err(|source| Error::Connect {
                endpoint: self.url.clone(),
                source,
            })
    }

    async fn subscribe(
        &self,
        channel: Channel,
        request: &SubscribePreconfsRequest,
    ) -> Result<Streaming<PreconfStreamMessage>, Error> {
        let mut client = PreconfServiceClient::new(channel);
        let mut call = Request::new(request.clone());
        call.metadata_mut().insert("x-token", self.token.clone());
        client
            .subscribe_preconfs(call)
            .await
            .map(|response| response.into_inner())
            .map_err(Error::from_status)
    }

    async fn open(
        &self,
        request: &SubscribePreconfsRequest,
    ) -> Result<Streaming<PreconfStreamMessage>, Error> {
        self.subscribe(self.channel().await?, request).await
    }
}

/// rustls installs no default provider when a binary pulls in more than one
fn install_crypto_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
}
