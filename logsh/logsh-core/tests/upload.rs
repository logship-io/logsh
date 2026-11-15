use logsh_core::{connect::Connection, error::UploadError, upload};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

struct TestFile(PathBuf);
impl TestFile {
    fn new(extension: &str, contents: &[u8], length: u64) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "logsh-upload-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            extension
        ));
        let mut file = File::create(&path).unwrap();
        file.write_all(contents).unwrap();
        if length > 0 {
            file.set_len(length).unwrap();
        }
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn server(action: impl FnOnce(TcpStream) + Send + 'static) -> (Connection, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut connection = Connection::new(&format!("http://{}", listener.local_addr().unwrap()));
    connection.default_account = Some(uuid::Uuid::nil());
    let mut value = serde_json::to_value(connection).unwrap();
    value["auth"] = serde_json::json!({"Jwt":{"expires":null,"token":"test-token"}});
    let connection = serde_json::from_value(value).unwrap();
    let thread = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        action(stream);
    });
    (connection, thread)
}

fn headers(stream: &mut TcpStream) -> String {
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 64 * 1024);
    }
    String::from_utf8(header).unwrap()
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    // Keep the response deliverable even if request bytes are already in flight.
    let mut buffer = [0; 8192];
    while let Ok(n) = stream.read(&mut buffer) {
        if n == 0 {
            break;
        }
    }
}

#[test]
fn successful_upload_preserves_file_headers_authentication_and_content_length() {
    let content = "name\tregistered\nSynthetic λ\t2026-10-26\n".as_bytes();
    let file = TestFile::new("TSV", content, 0);
    let (connection, worker) = server(move |mut stream| {
        let header = headers(&mut stream).to_ascii_lowercase();
        assert!(header
            .starts_with("post /inflow/00000000-0000-0000-0000-000000000000/example/tsv http/1.1"));
        assert!(header.contains("authorization: bearer test-token\r\n"));
        assert!(header.contains("content-type: text/tab-separated-values\r\n"));
        assert!(header.contains(&format!("content-length: {}\r\n", content.len())));
        assert!(!header.contains("transfer-encoding:"));
        assert!(header.contains("user-agent: logsh-core/"));
        assert!(header.contains("x-ls-hostname:"));
        let mut received = vec![0; content.len()];
        stream.read_exact(&mut received).unwrap();
        assert_eq!(received, content);
        respond(&mut stream, 200, "");
    });
    upload::execute(
        "example",
        file.path(),
        &connection,
        Some(Duration::from_secs(5)),
        false,
    )
    .unwrap();
    worker.join().unwrap();
}

#[test]
fn early_rejections_preserve_http_status_and_server_message_during_large_upload() {
    for (status, body, expected) in [
        (
            400,
            r#"{"message":"Invalid value for column 'registr_dt' on line 2040."}"#,
            "Invalid value for column 'registr_dt' on line 2040.",
        ),
        (401, "", "Unauthorized"),
        (
            413,
            "Upload exceeds proxy limit",
            "Upload exceeds proxy limit",
        ),
        (
            503,
            r#"{"message":"Storage capacity unavailable; ingestion is paused."}"#,
            "Storage capacity unavailable; ingestion is paused.",
        ),
    ] {
        let file = TestFile::new("tsv", b"id\tvalue\n", 128 * 1024 * 1024);
        let (connection, worker) = server(move |mut stream| {
            headers(&mut stream);
            respond(&mut stream, status, body);
        });
        let error = upload::execute(
            "example",
            file.path(),
            &connection,
            Some(Duration::from_secs(5)),
            false,
        )
        .unwrap_err();
        assert!(
            matches!(&error, UploadError::UploadFailureStatus(actual, message) if *actual == i32::from(status) && message == expected),
            "{error:?}"
        );
        worker.join().unwrap();
    }
}

#[test]
fn timeout_includes_underlying_cause_and_read_count() {
    let file = TestFile::new("csv", b"id\n1\n", 0);
    let (connection, worker) = server(|mut stream| {
        headers(&mut stream);
        thread::sleep(Duration::from_millis(300));
    });
    let error = upload::execute(
        "example",
        file.path(),
        &connection,
        Some(Duration::from_millis(100)),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(&error, UploadError::Interrupted { total_bytes: 5, source, details, .. } if source.is_timeout() && details.contains("timed out")),
        "{error:?}"
    );
    worker.join().unwrap();
}

#[test]
fn early_success_does_not_report_partial_file_as_uploaded() {
    let file = TestFile::new("tsv", b"id\tvalue\n", 128 * 1024 * 1024);
    let (connection, worker) = server(|mut stream| {
        headers(&mut stream);
        respond(&mut stream, 200, "");
    });
    let error = upload::execute(
        "example",
        file.path(),
        &connection,
        Some(Duration::from_secs(5)),
        false,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("before the complete file was read"),
        "{error:?}"
    );
    worker.join().unwrap();
}

#[test]
fn rejects_unsupported_extensions_without_contacting_server() {
    let file = TestFile::new("json", b"[]", 0);
    let connection = Connection::new("http://127.0.0.1:1");
    assert!(matches!(
        upload::execute("example", file.path(), &connection, None, false),
        Err(UploadError::UnsupportedFileExtension(_))
    ));
}

#[test]
fn explicit_null_markers_are_url_encoded_and_file_bytes_are_unchanged() {
    let content = b"registr_dt\n##/##/####\n";
    let file = TestFile::new("tsv", content, 0);
    let markers = [
        "##/##/####".to_owned(),
        "N/A & unknown".to_owned(),
        String::new(),
    ];
    let expected = markers.clone();
    let (connection, worker) = server(move |mut stream| {
        let header = headers(&mut stream);
        let request_path = header.split_whitespace().nth(1).unwrap();
        let url =
            logsh_core::reqwest::Url::parse(&format!("http://localhost{request_path}")).unwrap();
        assert_eq!(
            url.path(),
            "/inflow/00000000-0000-0000-0000-000000000000/example/tsv"
        );
        let received_markers: Vec<_> = url
            .query_pairs()
            .map(|(key, value)| {
                assert_eq!(key, "nullValue");
                value.into_owned()
            })
            .collect();
        assert_eq!(received_markers, expected);
        let mut body = vec![0; content.len()];
        stream.read_exact(&mut body).unwrap();
        assert_eq!(body, content);
        respond(&mut stream, 200, "");
    });
    upload::UploadOptions {
        timeout: Some(Duration::from_secs(5)),
        null_values: &markers,
        ..Default::default()
    }
    .execute("example", file.path(), &connection)
    .unwrap();
    worker.join().unwrap();
}
