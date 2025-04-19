use bytes::Bytes;
use flv::{data::FlvData, error::FlvError, parser_async::FlvDecoderStream};
use futures::{Stream, StreamExt};
use reqwest::{Client, Url};
use rustls::{ClientConfig, crypto::ring};
use rustls_platform_verifier::BuilderVerifierExt;
use std::pin::Pin;

use std::sync::Arc;
use tracing::{debug, info};

use crate::{
    DownloadError, DownloaderConfig, downloader::BytesStreamReader, proxy::build_proxy_from_config,
    utils::format_bytes,
};

// Type alias for a boxed stream of FLV data
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T, FlvError>> + Send>>;

// Type alias for a boxed stream of raw bytes
pub type RawByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// FLV Downloader for streaming FLV content from URLs
pub struct FlvDownloader {
    client: Client,
    config: DownloaderConfig,
}

impl FlvDownloader {
    /// Create a new FlvDownloader with default configuration
    pub fn new() -> Result<Self, DownloadError> {
        Self::with_config(DownloaderConfig::default())
    }

    /// Create a new FlvDownloader with custom configuration
    pub fn with_config(config: DownloaderConfig) -> Result<Self, DownloadError> {
        // Create the crypto provider
        let provider = Arc::new(ring::default_provider());

        // Build platform default TLS configuration
        let tls_config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("Failed to configure default TLS protocol versions")
            .with_platform_verifier()
            .with_no_client_auth();

        let mut client_builder = Client::builder()
            .pool_max_idle_per_host(5) // Allow multiple connections to same host
            .user_agent(&config.user_agent)
            .default_headers(config.headers.clone())
            .use_preconfigured_tls(tls_config)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            });

        if !config.timeout.is_zero() {
            client_builder = client_builder.timeout(config.timeout);
        }

        if !config.connect_timeout.is_zero() {
            client_builder = client_builder.connect_timeout(config.connect_timeout);
        }

        if !config.read_timeout.is_zero() {
            client_builder = client_builder.pool_idle_timeout(config.read_timeout);
        }

        // Set up proxy configuration
        if let Some(proxy_config) = &config.proxy {
            // Explicit proxy configuration takes precedence
            let proxy = match build_proxy_from_config(proxy_config) {
                Ok(p) => p,
                Err(e) => return Err(DownloadError::ProxyError(e)),
            };
            client_builder = client_builder.proxy(proxy);
            info!(proxy_url = %proxy_config.url, "Using explicitly configured proxy for downloads");
        } else if config.use_system_proxy {
            // No explicit proxy but system proxy enabled
            // reqwest will use system proxy settings by default when we don't call no_proxy()
            info!("Using system proxy settings for downloads");
        } else {
            // Explicitly disable proxy
            client_builder = client_builder.no_proxy();
            debug!("Proxy disabled for downloads");
        }

        let client = client_builder.build()?;

        Ok(Self { client, config })
    }

    /// Download a stream from a URL string and return an FLV data stream
    pub async fn download(&self, url_str: &str) -> Result<BoxStream<FlvData>, DownloadError> {
        let url = url_str
            .parse::<Url>()
            .map_err(|_| DownloadError::UrlError(url_str.to_string()))?;
        self.download_url(url).await
    }

    /// Download a stream from a URL string and return a raw byte stream without parsing
    pub async fn download_raw(&self, url_str: &str) -> Result<RawByteStream, DownloadError> {
        let url = url_str
            .parse::<Url>()
            .map_err(|_| DownloadError::UrlError(url_str.to_string()))?;
        self.download_url_raw(url).await
    }

    /// Download a stream from a URL and return an FLV data stream
    pub async fn download_url(&self, url: Url) -> Result<BoxStream<FlvData>, DownloadError> {
        info!(url = %url, "Starting FLV download");

        // Start the request
        let response = self.client.get(url.clone()).send().await?;

        // Check response status
        if !response.status().is_success() {
            return Err(DownloadError::StatusCode(response.status()));
        }

        // Log file size if available
        if let Some(content_length) = response.content_length() {
            info!(
                url = %url,
                size = %format_bytes(content_length),
                "Download size information available"
            );
        } else {
            debug!(url = %url, "Content length not available");
        }

        // Get the bytes stream from the response
        let bytes_stream = response.bytes_stream();

        // Wrap the bytes stream in our adapter
        let reader = BytesStreamReader::new(bytes_stream);

        // Create a buffered reader with the specified buffer size
        let buffered_reader = tokio::io::BufReader::with_capacity(self.config.buffer_size, reader);

        // Create the FLV decoder stream
        let decoder_stream =
            FlvDecoderStream::with_capacity(buffered_reader, self.config.buffer_size);

        // Box the stream and return it
        Ok(decoder_stream.boxed())
    }

    /// Download a stream from a URL and return a raw byte stream without parsing
    pub async fn download_url_raw(&self, url: Url) -> Result<RawByteStream, DownloadError> {
        info!(url = %url, "Starting raw download (no FLV parsing)");

        // Start the request
        let response = self.client.get(url.clone()).send().await?;

        // Check response status
        if !response.status().is_success() {
            return Err(DownloadError::StatusCode(response.status()));
        }

        // Log file size if available
        if let Some(content_length) = response.content_length() {
            info!(
                url = %url,
                size = %format_bytes(content_length),
                "Download size information available"
            );
        } else {
            debug!(url = %url, "Content length not available");
        }

        // Get the bytes stream from the response
        let bytes_stream = response.bytes_stream();

        // Transform the reqwest bytes stream into our raw byte stream
        let raw_stream = bytes_stream
            .map(|result| result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
            .boxed();

        Ok(raw_stream)
    }
}
