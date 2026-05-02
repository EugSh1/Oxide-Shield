use crate::shield::{
    config::{Config, ServerConfig},
    rate_limiter::RateLimiter,
};
use anyhow::{Context, Result, anyhow};
use http_body_util::{Either, Full};
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    client::conn::http1 as client_http1,
    header::{self, HeaderValue},
    server::conn::http1 as server_http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream};

const BUCKET_CAPACITY_HEADER_NAME: &str = "X-RateLimit-Limit";
const REMAINING_TOKENS_HEADER_NAME: &str = "X-RateLimit-Remaining";

type GenericResponse = Response<Either<Full<Bytes>, Incoming>>;

struct InnerState {
    config: ServerConfig,
    rate_limiter: RateLimiter,
}

#[derive(Clone)]
pub struct Server {
    inner: Arc<InnerState>,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(InnerState {
                config: config.server,
                rate_limiter: RateLimiter::new(config.rate_limiter),
            }),
        }
    }

    pub async fn run(self) -> Result<()> {
        tracing::info!(
            ip_header_name = ?self.inner.config.ip_header_name,
            "Oxide-Shield started"
        );

        let listener = TcpListener::bind(self.inner.config.bind_addr).await?;

        loop {
            let (stream, client_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(error) => {
                    tracing::warn!(error = ?error, "Failed to accept connection");
                    continue;
                }
            };

            let io = TokioIo::new(stream);

            let server = self.clone();

            tokio::task::spawn(async move {
                let service = |req: Request<Incoming>| server.handle_connection(req, client_addr);

                if let Err(error) = server_http1::Builder::new()
                    .serve_connection(io, service_fn(service))
                    .await
                {
                    tracing::error!(error = ?error, "Failed to handle connection");
                }
            });
        }
    }

    async fn handle_connection(
        &self,
        req: Request<Incoming>,
        client_addr: SocketAddr,
    ) -> Result<GenericResponse> {
        let client_ip = self.get_real_client_ip(&req, client_addr.ip());

        let (is_rate_limited, remaining_tokens) =
            self.inner.rate_limiter.is_rate_limited(client_ip);

        if is_rate_limited {
            return self.too_many_requests();
        }

        match self.proxy(req, remaining_tokens, client_ip).await {
            Ok(res) => Ok(res),
            Err(error) => {
                tracing::error!(error = ?error, "Backend is down or unreachable");
                self.bad_gateway()
            }
        }
    }

    /// Handles the request by proxying it to the backend server.
    async fn proxy(
        &self,
        mut req: Request<Incoming>,
        remaining_tokens: f64,
        client_ip: IpAddr,
    ) -> Result<GenericResponse> {
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(&self.inner.config.server_addr),
        )
        .await
        .map_err(|_| anyhow!("Backend connect timeout"))??;

        let io = TokioIo::new(stream);

        let (mut sender, conn) = client_http1::Builder::new()
            .handshake::<_, Incoming>(io)
            .await?;

        tokio::spawn(async move {
            if let Err(error) = conn.await {
                tracing::warn!(error = ?error, "Backend connection error");
            }
        });

        req.headers_mut().insert(
            header::HOST,
            HeaderValue::from_str(&self.inner.config.server_addr)?,
        );

        req.headers_mut().insert(
            "X-Forwarded-For",
            HeaderValue::from_str(&client_ip.to_string())?,
        );

        let res = tokio::time::timeout(Duration::from_secs(30), sender.send_request(req))
            .await
            .context("Backend request timeout")??;

        let (mut parts, body) = res.into_parts();

        parts.headers.insert(
            REMAINING_TOKENS_HEADER_NAME,
            HeaderValue::from(remaining_tokens as u64),
        );

        parts.headers.insert(
            BUCKET_CAPACITY_HEADER_NAME,
            HeaderValue::from(self.inner.rate_limiter.buckets_capacity() as u64),
        );

        Ok(Response::from_parts(parts, Either::Right(body)))
    }

    /// Gets the real client IP from the request headers.
    /// If the IP header is not present, the client IP is returned.
    fn get_real_client_ip(&self, req: &Request<Incoming>, client_ip: IpAddr) -> IpAddr {
        let ip_header_name = self.inner.config.ip_header_name.as_ref();

        ip_header_name
            .and_then(|name| req.headers().get(name))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<IpAddr>().ok())
            .unwrap_or(client_ip)
    }

    /// Returns a response with status code 429 (Too Many Requests).
    fn too_many_requests(&self) -> Result<GenericResponse> {
        let body = Either::Left(Full::new(Bytes::from_static(b"429 Too Many Requests")));

        Ok(Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::CONTENT_TYPE, "text/plain")
            .header(REMAINING_TOKENS_HEADER_NAME, 0)
            .header(
                BUCKET_CAPACITY_HEADER_NAME,
                self.inner.rate_limiter.buckets_capacity() as u64,
            )
            .body(body)?)
    }

    /// Returns a response with status code 502 (Bad Gateway).
    fn bad_gateway(&self) -> Result<GenericResponse> {
        let body = Either::Left(Full::new(Bytes::from_static(b"502 Bad Gateway")));

        Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(body)?)
    }
}
