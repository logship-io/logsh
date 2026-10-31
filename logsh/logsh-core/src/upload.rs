use std::{
    error::Error,
    fs::File,
    io::Write,
    path::Path,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use crate::{
    connect::Connection,
    error::{CommonError, UploadError},
};
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use serde_json::Value;
use tokio::io::{AsyncRead, ReadBuf};

struct UploadBody {
    file: tokio::fs::File,
    buffer: Box<[u8; 65536]>,
    sent: Arc<AtomicU64>,
    total: u64,
    remaining: u64,
    last_percent: u64,
    pretty: bool,
}

impl Body for UploadBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        if this.remaining == 0 {
            return Poll::Ready(None);
        }
        let capacity = this.remaining.min(this.buffer.len() as u64) as usize;
        let mut buffer = ReadBuf::new(&mut this.buffer[..capacity]);
        match Pin::new(&mut this.file).poll_read(cx, &mut buffer) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Some(Err(error))),
            Poll::Ready(Ok(())) => {
                let bytes = buffer.filled().len() as u64;
                if bytes == 0 {
                    return Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "File ended before its advertised length.",
                    ))));
                }
                let frame = Frame::data(Bytes::copy_from_slice(buffer.filled()));
                this.remaining -= bytes;
                let sent = this.sent.fetch_add(bytes, Ordering::Relaxed) + bytes;
                if this.pretty && this.total > 0 {
                    let percent =
                        ((u128::from(sent) * 100) / u128::from(this.total)).min(100) as u64;
                    if percent != this.last_percent {
                        eprint!("\rUploading: {percent:>3}%");
                        let _ = std::io::stderr().flush();
                        this.last_percent = percent;
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
        }
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.remaining)
    }
    fn is_end_stream(&self) -> bool {
        self.remaining == 0
    }
}

fn error_details(error: &(dyn Error + 'static)) -> String {
    let mut details = error.to_string();
    let mut cause = error.source();
    while let Some(error) = cause {
        details.push_str(": ");
        details.push_str(&error.to_string());
        cause = error.source();
    }
    details
}

async fn map_upload_error(response: reqwest::Response) -> UploadError {
    let status = response.status();
    let message = match response.text().await {
        Ok(body) => serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or(body),
        Err(error) => format!("Could not read error response: {}", error_details(&error)),
    };
    let message = if message.trim().is_empty() {
        status
            .canonical_reason()
            .unwrap_or("Empty error response")
            .to_owned()
    } else {
        message
    };
    UploadError::UploadFailureStatus(i32::from(status.as_u16()), message)
}

/// Streams a file without waiting for the request body to finish before handling
/// an early server response. Blocking reqwest's reader body can hide that response
/// behind a closed body-channel error when the server rejects a large upload.
pub fn execute(
    schema_str: &str,
    path_str: &str,
    connection: &Connection,
    timeout: Option<std::time::Duration>,
    pretty_progress: bool,
) -> Result<(), UploadError> {
    UploadOptions {
        timeout,
        pretty_progress,
        ..Default::default()
    }
    .execute(schema_str, path_str, connection)
}

/// CSV/TSV upload settings. Null markers are opt-in and matched by the server
/// after unquoting fields; file contents are streamed unchanged.
#[derive(Default)]
pub struct UploadOptions<'a> {
    pub timeout: Option<std::time::Duration>,
    pub pretty_progress: bool,
    pub null_values: &'a [String],
}

impl UploadOptions<'_> {
    pub fn execute(
        &self,
        schema_str: &str,
        path_str: &str,
        connection: &Connection,
    ) -> Result<(), UploadError> {
        let timeout = self.timeout;
        let pretty_progress = self.pretty_progress;
        if path_str.trim().is_empty() {
            return Err(UploadError::Common(CommonError::EmptyArgument(
                "path".to_string(),
            )));
        }
        let path = Path::new(path_str);
        if !path.exists() {
            return Err(UploadError::Common(CommonError::FileNotFound(
                path_str.to_owned(),
            )));
        }
        let ext = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
            .ok_or_else(|| UploadError::UnsupportedFileExtension(String::new()))?;
        let content_type = match ext.as_str() {
            "csv" => "text/csv",
            "tsv" => "text/tab-separated-values",
            _ => return Err(UploadError::UnsupportedFileExtension(ext)),
        };
        let account = connection.effective_account()?.ok_or(UploadError::Config(
            crate::error::ConfigError::NoDefaultAccount,
        ))?;
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let sent = Arc::new(AtomicU64::new(0));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let result = runtime.block_on(async {
            let mut builder = reqwest::Client::builder()
                .default_headers(crate::connect::client_headers());
            if let Some(timeout) = timeout {
                builder = builder.timeout(timeout);
            }
            let client = builder.build()?;
            let mut request = client.post(format!(
                "{}/inflow/{}/{}/{}",
                connection.server.trim_end_matches('/'), account, schema_str, ext
            ));
            if let Some(token) = connection.get_token() {
                request = request.bearer_auth(token);
            }
            let body = UploadBody {
                file: tokio::fs::File::from_std(file),
                buffer: Box::new([0; 65536]),
                sent: Arc::clone(&sent),
                total: file_size,
                remaining: file_size,
                last_percent: 0,
                pretty: pretty_progress,
            };
            let mut request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .header(reqwest::header::CONTENT_LENGTH, file_size)
                .body(reqwest::Body::wrap(body))
                .build()?;
            for value in self.null_values {
                request.url_mut().query_pairs_mut().append_pair("nullValue", value);
            }
            let response = client.execute(request).await.map_err(|source| UploadError::Interrupted {
                bytes_read: sent.load(Ordering::Relaxed),
                total_bytes: file_size,
                details: error_details(&source),
                source,
            })?;
            if !response.status().is_success() {
                return Err(map_upload_error(response).await);
            }
            if sent.load(Ordering::Relaxed) != file_size {
                return Err(UploadError::Common(CommonError::IOError(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("Server returned success before the complete file was read ({} of {file_size} bytes); ingestion may be partial.", sent.load(Ordering::Relaxed)),
                ))));
            }
            Ok(())
        });
        if pretty_progress {
            eprintln!();
        }
        result
    }
}
