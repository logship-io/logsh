use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
pub struct Expected {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
    pub status: u16,
    pub response: Value,
}
pub struct Server {
    pub url: String,
    worker: thread::JoinHandle<()>,
}
impl Server {
    pub fn new(expected: Vec<Expected>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server_url = url.clone();
        let worker = thread::spawn(move || {
            for expected in expected {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "Missing request {} {}",
                                expected.method,
                                expected.path
                            );
                            thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buf = [0u8; 4096];
                let (head_end, content_length) = loop {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..index]);
                        let length = header
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        break (index + 4, length);
                    }
                };
                while bytes.len() < head_end + content_length {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                }
                let header = String::from_utf8_lossy(&bytes[..head_end]);
                assert!(
                    header.starts_with(&format!("{} {} HTTP/1.1", expected.method, expected.path)),
                    "{header}"
                );
                if !expected.path.starts_with("/auth/")
                    && !expected.path.starts_with("/device")
                    && !expected.path.starts_with("/token")
                {
                    assert!(header
                        .to_lowercase()
                        .contains("authorization: bearer test-token"));
                }
                if let Some(body) = expected.body {
                    assert_eq!(
                        serde_json::from_slice::<Value>(
                            &bytes[head_end..head_end + content_length]
                        )
                        .unwrap(),
                        body
                    );
                }
                let body = if expected.status == 204 {
                    String::new()
                } else {
                    expected
                        .response
                        .to_string()
                        .replace("$SERVER", &server_url)
                };
                write!(stream,"HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",expected.status,body.len(),body).unwrap();
            }
        });
        Self { url, worker }
    }
    pub fn verify(self) {
        self.worker.join().unwrap();
    }
}
pub fn request(
    method: &'static str,
    path: impl Into<String>,
    body: Option<Value>,
    response: Value,
) -> Expected {
    Expected {
        method,
        path: path.into(),
        body,
        status: 200,
        response,
    }
}
