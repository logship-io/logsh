use logsh_core::{
    connect::Connection,
    error::UploadError,
    upload::{UploadEncoding, UploadOptions},
};
use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::Duration,
};

struct TestFile(PathBuf);
impl TestFile {
    fn new(content: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "logsh-columnar-{}-{:?}.csv",
            std::process::id(),
            thread::current().id()
        ));
        fs::write(&path, content).unwrap();
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

fn setup<T: Send + 'static>(
    action: impl FnOnce(TcpListener) -> T + Send + 'static,
) -> (Connection, thread::JoinHandle<T>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut connection = Connection::new(&format!("http://{}", listener.local_addr().unwrap()));
    connection.default_account = Some(uuid::Uuid::nil());
    (connection, thread::spawn(move || action(listener)))
}
fn accept(listener: &TcpListener) -> (TcpStream, String) {
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    (
        stream,
        String::from_utf8(bytes).unwrap().to_ascii_lowercase(),
    )
}
fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let _ = stream.shutdown(Shutdown::Write);
}
fn line(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).unwrap()
}
fn body(stream: &mut TcpStream, headers: &str) -> Vec<u8> {
    if headers.contains("transfer-encoding: chunked") {
        let mut result = Vec::new();
        loop {
            let length =
                usize::from_str_radix(line(stream).trim().split(';').next().unwrap(), 16).unwrap();
            if length == 0 {
                assert_eq!(line(stream), "\r\n");
                return result;
            }
            let start = result.len();
            result.resize(start + length, 0);
            stream.read_exact(&mut result[start..]).unwrap();
            assert_eq!(line(stream), "\r\n");
        }
    }
    let size: usize = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .unwrap()
        .parse()
        .unwrap();
    let mut result = vec![0; size];
    stream.read_exact(&mut result).unwrap();
    result
}
fn capabilities() -> &'static str {
    r#"{"version":1,"maximumRecordBytes":1048576,"maximumBatchBytes":1048576,"maximumBatchRows":1024,"invariantNumberInference":true,"columns":[]}"#
}

#[test]
fn auto_falls_back_before_uploading_when_server_has_no_capability_endpoint() {
    let contents = b"message,count\nunchanged,123\n";
    let file = TestFile::new(contents);
    let (connection, worker) = setup(move |listener| {
        let (mut probe, header) = accept(&listener);
        assert!(header.starts_with("get "));
        assert!(header.contains("/encoding "));
        respond(&mut probe, 404, "");
        drop(probe);
        let (mut upload, header) = accept(&listener);
        assert!(header.starts_with("post "));
        assert!(header.contains("/csv "));
        assert_eq!(body(&mut upload, &header), contents);
        respond(&mut upload, 200, "");
    });
    let stats = UploadOptions {
        encoding: UploadEncoding::Auto,
        timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    }
    .execute_with_stats("logs", file.path(), &connection)
    .unwrap();
    assert_eq!(stats.encoding, "raw");
    assert_eq!(stats.wire_bytes, contents.len() as u64);
    worker.join().unwrap();
}

#[test]
fn negotiated_upload_streams_gzip_with_a_versioned_body_and_reports_wire_bytes() {
    let contents =
        "message,count,enabled\n".to_owned() + &"\"hello\nworld\",1234567890,true\n".repeat(1000);
    let file = TestFile::new(contents.as_bytes());
    let (connection, worker) = setup(move |listener| {
        let (mut probe, _) = accept(&listener);
        respond(&mut probe, 200, capabilities());
        drop(probe);
        let (mut upload, header) = accept(&listener);
        assert!(header.contains("/columnar "));
        assert!(header.contains("content-encoding: gzip"));
        assert!(header.contains("content-type: application/vnd.logship.columnar.v1"));
        let wire = body(&mut upload, &header);
        let mut decoded = Vec::new();
        flate2::read::GzDecoder::new(wire.as_slice())
            .read_to_end(&mut decoded)
            .unwrap();
        assert!(decoded.starts_with(b"LSCB001\n"));
        assert!(decoded.ends_with(&[0; 8]));
        respond(&mut upload, 200, "");
        wire.len()
    });
    let stats = UploadOptions {
        encoding: UploadEncoding::Auto,
        timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    }
    .execute_with_stats("logs", file.path(), &connection)
    .unwrap();
    assert_eq!(stats.encoding, "columnar-v1+gzip");
    assert_eq!(stats.source_bytes, contents.len() as u64);
    assert_eq!(stats.wire_bytes, worker.join().unwrap() as u64);
    assert!(stats.wire_bytes * 10 < stats.source_bytes);
}

#[test]
fn early_server_errors_stop_the_encoder_and_keep_the_original_response() {
    let file = TestFile::new(format!("message\n{}", "valid-payload\n".repeat(200_000)).as_bytes());
    let (connection, worker) = setup(move |listener| {
        let (mut probe, _) = accept(&listener);
        respond(&mut probe, 200, capabilities());
        drop(probe);
        let (mut upload, _) = accept(&listener);
        respond(&mut upload, 413, "{\"message\":\"original rejection\"}");
        let mut buffer = [0; 8192];
        while let Ok(count) = upload.read(&mut buffer) {
            if count == 0 {
                break;
            }
        }
    });
    let error = UploadOptions {
        encoding: UploadEncoding::Columnar,
        timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    }
    .execute("logs", file.path(), &connection)
    .unwrap_err();
    assert!(
        matches!(error, UploadError::UploadFailureStatus(413, ref message) if message == "original rejection")
    );
    worker.join().unwrap();
}

#[test]
fn auto_samples_encoded_columns_and_can_skip_expensive_low_gain_gzip() {
    for (mixed, encoding, compressed) in [
        (true, UploadEncoding::Auto, false),
        (true, UploadEncoding::Columnar, true),
        (false, UploadEncoding::Auto, true),
    ] {
        let mut contents = b"number,message,a,b,c,d,e,f,g,h,i,j\n".to_vec();
        let mut state = 19u32;
        let mut repeated = [0u8; 500];
        let alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        for byte in &mut repeated {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = alphabet[(state & 63) as usize];
        }
        for row in 0..1024 {
            contents.extend_from_slice(row.to_string().as_bytes());
            contents.push(b',');
            if mixed {
                for _ in 0..500 {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    contents.push(alphabet[(state & 63) as usize]);
                }
            } else {
                contents.extend_from_slice(&repeated);
            }
            contents.extend_from_slice(b",1,2,3,4,5,6,7,8,9,10\n");
        }
        let file = TestFile::new(&contents);
        let (connection, worker) = setup(move |listener| {
            let (mut probe, _) = accept(&listener);
            respond(&mut probe, 200, capabilities());
            drop(probe);
            let (mut upload, headers) = accept(&listener);
            assert!(headers.contains("/columnar "));
            assert_eq!(headers.contains("content-encoding: gzip"), compressed);
            let wire = body(&mut upload, &headers);
            let mut decoded = Vec::new();
            if compressed {
                flate2::read::GzDecoder::new(wire.as_slice())
                    .read_to_end(&mut decoded)
                    .unwrap();
            } else {
                decoded.extend_from_slice(&wire);
            }
            assert!(decoded.starts_with(b"LSCB001\n"));
            assert!(decoded.ends_with(&[0; 8]));
            respond(&mut upload, 200, "");
            wire.len()
        });
        let stats = UploadOptions {
            encoding,
            timeout: Some(Duration::from_secs(10)),
            ..Default::default()
        }
        .execute_with_stats("logs", file.path(), &connection)
        .unwrap();
        assert_eq!(stats.source_bytes, contents.len() as u64); // Sampling must rewind the file.
        assert_eq!(
            stats.encoding,
            if compressed {
                "columnar-v1+gzip"
            } else {
                "columnar-v1"
            }
        );
        assert_eq!(stats.wire_bytes, worker.join().unwrap() as u64);
    }
}
