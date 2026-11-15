use bytes::Bytes;
use flate2::{write::GzEncoder, Compression};
use http_body::{Body, Frame};
use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    task::{Context, Poll, Waker},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Capabilities {
    pub version: u32,
    pub maximum_record_bytes: usize,
    pub maximum_batch_bytes: usize,
    pub maximum_batch_rows: usize,
    pub invariant_number_inference: bool,
    pub columns: Vec<DeclaredColumn>,
}
#[derive(Debug, Deserialize)]
pub(super) struct DeclaredColumn {
    pub name: String,
    pub r#type: String,
}
impl Capabilities {
    pub fn supported(&self) -> bool {
        self.version == 1
            && self.maximum_record_bytes > 0
            && self.maximum_record_bytes <= 16 * 1024 * 1024
            && self.maximum_batch_bytes >= 1024
            && self.maximum_batch_bytes <= 16 * 1024 * 1024
            && self.maximum_batch_rows > 0
            && self.maximum_batch_rows <= 1024
    }
}

#[derive(Default)]
struct QueueState {
    chunks: VecDeque<Bytes>,
    closed: bool,
    cancelled: bool,
    finished: bool,
    error: Option<String>,
    error_emitted: bool,
    wake: Option<Waker>,
}
#[derive(Default)]
pub(super) struct Transfer {
    state: Mutex<QueueState>,
    room: Condvar,
    pub wire_bytes: AtomicU64,
}
impl Transfer {
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.chunks.is_empty() {
            state.finished = false;
        }
        state.cancelled = true;
        state.closed = true;
        state.chunks.clear();
        self.room.notify_all();
        if let Some(wake) = state.wake.take() {
            wake.wake();
        }
    }
    pub fn finished(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.finished && state.chunks.is_empty()
    }
    pub fn error(&self) -> Option<String> {
        self.state.lock().unwrap().error.clone()
    }
    fn check(&self) -> io::Result<()> {
        if self.state.lock().unwrap().cancelled {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "Upload stopped"))
        } else {
            Ok(())
        }
    }
    fn push(&self, bytes: Bytes, finished: bool) -> io::Result<()> {
        let mut state = self.state.lock().unwrap();
        while !bytes.is_empty() && state.chunks.len() >= 2 && !state.cancelled {
            state = self.room.wait(state).unwrap();
        }
        if state.cancelled {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Upload stopped"));
        }
        if !bytes.is_empty() {
            state.chunks.push_back(bytes);
        }
        if finished {
            state.finished = true;
            state.closed = true;
        }
        if let Some(wake) = state.wake.take() {
            wake.wake();
        }
        Ok(())
    }
    fn close(&self, result: io::Result<()>) {
        let mut state = self.state.lock().unwrap();
        if !state.cancelled {
            state.finished = result.is_ok();
            state.error = result.err().map(|error| error.to_string());
            state.closed = true;
            if let Some(wake) = state.wake.take() {
                wake.wake();
            }
        }
    }
}

pub(super) struct UploadBody {
    transfer: Arc<Transfer>,
}
impl Body for UploadBody {
    type Data = Bytes;
    type Error = io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let mut state = self.transfer.state.lock().unwrap();
        if let Some(bytes) = state.chunks.pop_front() {
            self.transfer.room.notify_one();
            self.transfer
                .wire_bytes
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
            return Poll::Ready(Some(Ok(Frame::data(bytes))));
        }
        if !state.error_emitted {
            if let Some(message) = state.error.clone() {
                state.error_emitted = true;
                return Poll::Ready(Some(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    message,
                ))));
            }
        }
        if state.closed {
            Poll::Ready(None)
        } else {
            state.wake = Some(cx.waker().clone());
            Poll::Pending
        }
    }
    fn is_end_stream(&self) -> bool {
        let state = self.transfer.state.lock().unwrap();
        state.closed && state.chunks.is_empty() && (state.error.is_none() || state.error_emitted)
    }
}
impl Drop for UploadBody {
    fn drop(&mut self) {
        // A server may reject a request before its producer has finished. Wake a producer blocked by backpressure.
        let finished = self.transfer.finished();
        if !finished {
            self.transfer.cancel();
        }
    }
}

struct QueueWriter {
    transfer: Arc<Transfer>,
    buffer: Vec<u8>,
}
impl QueueWriter {
    fn finish(mut self) -> io::Result<()> {
        self.transfer
            .push(Bytes::from(std::mem::take(&mut self.buffer)), true)
    }
}
impl Write for QueueWriter {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let length = bytes.len();
        self.transfer.check()?;
        while !bytes.is_empty() {
            let count = bytes.len().min(65536 - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.buffer.len() == 65536 {
                self.flush()?;
            }
        }
        Ok(length)
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            let bytes = std::mem::replace(&mut self.buffer, Vec::with_capacity(65536));
            self.transfer.push(Bytes::from(bytes), false)?;
        }
        Ok(())
    }
}

struct CountedFile {
    file: io::Take<File>,
    transfer: Arc<Transfer>,
    sent: Arc<AtomicU64>,
    total: u64,
    last_percent: u64,
    pretty: bool,
}
impl Read for CountedFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.transfer.check()?;
        let count = self.file.read(bytes)?;
        let sent = self.sent.fetch_add(count as u64, Ordering::Relaxed) + count as u64;
        if count == 0 && sent != self.total {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CSV file ended before its advertised length",
            ));
        }
        if self.pretty && self.total > 0 {
            let percent = ((u128::from(sent) * 100) / u128::from(self.total)).min(100) as u64;
            if percent != self.last_percent {
                eprint!("\rUploading: {percent:>3}%");
                let _ = io::stderr().flush();
                self.last_percent = percent;
            }
        }
        Ok(count)
    }
}

pub(super) struct Options {
    pub delimiter: u8,
    pub capabilities: Capabilities,
    pub null_values: Vec<String>,
    pub adaptive_compression: bool,
}

pub(super) fn body(
    mut file: File,
    options: Options,
    sent: Arc<AtomicU64>,
    total: u64,
    pretty: bool,
) -> io::Result<(UploadBody, Arc<Transfer>, bool)> {
    let Options {
        delimiter,
        capabilities,
        null_values,
        adaptive_compression,
    } = options;
    let compressed = !adaptive_compression
        || should_compress(&mut file, delimiter, &capabilities, &null_values)?;
    let transfer = Arc::new(Transfer::default());
    let output = Arc::clone(&transfer);
    std::thread::Builder::new()
        .name("logsh-upload-encoder".into())
        .spawn(move || {
            let reader = CountedFile {
                file: file.take(total),
                transfer: Arc::clone(&output),
                sent,
                total,
                last_percent: 0,
                pretty,
            };
            let writer = QueueWriter {
                transfer: Arc::clone(&output),
                buffer: Vec::with_capacity(65536),
            };
            let result = (|| {
                if compressed {
                    let mut gzip = GzEncoder::new(writer, Compression::fast());
                    encode(reader, &mut gzip, delimiter, &capabilities, &null_values)?;
                    gzip.finish()?.finish()
                } else {
                    let mut writer = writer;
                    encode(reader, &mut writer, delimiter, &capabilities, &null_values)?;
                    writer.finish()
                }
            })();
            output.close(result);
        })?;
    Ok((
        UploadBody {
            transfer: Arc::clone(&transfer),
        },
        transfer,
        compressed,
    ))
}

// Sample encoded columns, not raw character entropy: a repeated high-entropy string
// may still have an excellent column dictionary. Sampling never consumes upload input.
fn should_compress(
    file: &mut File,
    delimiter: u8,
    caps: &Capabilities,
    markers: &[String],
) -> io::Result<bool> {
    let position = file.stream_position()?;
    let result = (|| {
        let mut records = Records {
            input: BufReader::with_capacity(65536, &mut *file),
            line: 1,
            maximum: caps.maximum_record_bytes,
            scratch: Vec::new(),
        };
        let mut sample = Vec::new();
        let mut lines = 0;
        while let Some((_, record)) = records.next()? {
            sample.extend_from_slice(&record);
            sample.push(b'\n');
            records.recycle(record);
            lines += 1;
            if lines > 1 && (sample.len() >= 256 * 1024 || lines > caps.maximum_batch_rows) {
                break;
            }
        }
        let mut encoded = Vec::new();
        encode(sample.as_slice(), &mut encoded, delimiter, caps, markers)?;
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        gzip.write_all(&encoded)?;
        let compressed = gzip.finish()?;
        // Throughput policy: require at least 40% wire savings to pay for gzip.
        Ok(compressed.len() as u64 * 100 <= encoded.len() as u64 * 60)
    })();
    file.seek(SeekFrom::Start(position))?;
    result
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

struct Records<R> {
    input: BufReader<R>,
    line: u64,
    maximum: usize,
    scratch: Vec<u8>,
}
impl<R: Read> Records<R> {
    fn recycle(&mut self, mut value: Vec<u8>) {
        value.clear();
        self.scratch = value;
    }
    fn next(&mut self) -> io::Result<Option<(u64, Vec<u8>)>> {
        let start = self.line;
        let mut value = std::mem::take(&mut self.scratch);
        value.clear();
        let mut quoted = false;
        loop {
            let available = self.input.fill_buf()?;
            if available.is_empty() {
                if quoted {
                    return Err(invalid(format!(
                        "Unterminated quoted field on line {start}"
                    )));
                }
                if value.is_empty() {
                    return Ok(None);
                }
                break;
            }
            let mut consumed = 0;
            let mut ended = false;
            while let Some(next) = memchr::memchr2(b'"', b'\n', &available[consumed..]) {
                consumed += next + 1;
                if available[consumed - 1] == b'"' {
                    quoted = !quoted;
                } else {
                    self.line += 1;
                    if !quoted {
                        ended = true;
                        break;
                    }
                }
            }
            if !ended {
                consumed = available.len();
            }
            let bytes = consumed - usize::from(ended);
            if value.len().saturating_add(bytes) > self.maximum.saturating_add(1) {
                return Err(invalid(format!(
                    "Input record on line {start} exceeds {} bytes",
                    self.maximum
                )));
            }
            value.extend_from_slice(&available[..bytes]);
            self.input.consume(consumed);
            if ended {
                break;
            }
        }
        if value.last() == Some(&b'\r') {
            value.pop();
        }
        if value.len() > self.maximum {
            return Err(invalid(format!(
                "Input record on line {start} exceeds {} bytes",
                self.maximum
            )));
        }
        Ok(Some((start, value)))
    }
}

#[derive(Clone)]
struct Cell {
    state: u8,
    value: Vec<u8>,
}

// The hot path borrows fields from the record and copies them once into a reusable batch arena.
fn visit_fields<'a>(
    record: &'a [u8],
    delimiter: u8,
    mut visit: impl FnMut(usize, u8, std::borrow::Cow<'a, [u8]>) -> io::Result<()>,
) -> io::Result<usize> {
    let mut start = 0;
    let mut quoted = false;
    let mut count = 0;
    let mut offset = 0;
    loop {
        let position = memchr::memchr2(b'"', delimiter, &record[offset..])
            .map_or(record.len(), |next| offset + next);
        if position < record.len() && record[position] == b'"' {
            quoted = !quoted;
            offset = position + 1;
            continue;
        }
        if position < record.len() && quoted {
            offset = position + 1;
            continue;
        }
        let mut value = &record[start..position];
        let was_empty = value.is_empty();
        let mut was_quoted = false;
        if value.len() >= 2 && value[0] == b'"' && value[value.len() - 1] == b'"' {
            value = &value[1..value.len() - 1];
            was_quoted = true;
        }
        let value = if was_quoted && value.contains(&b'"') {
            let mut decoded = Vec::with_capacity(value.len());
            let mut index = 0;
            while index < value.len() {
                let byte = value[index];
                decoded.push(byte);
                index += 1;
                if byte == b'"' && value.get(index) == Some(&b'"') {
                    index += 1;
                }
            }
            std::borrow::Cow::Owned(decoded)
        } else {
            std::borrow::Cow::Borrowed(value)
        };
        let state = if was_empty {
            0
        } else if value.is_empty() {
            1
        } else {
            2
        };
        visit(count, state, value)?;
        count += 1;
        if position == record.len() {
            break;
        }
        offset = position + 1;
        start = offset;
    }
    Ok(count)
}

fn fields(record: &[u8], delimiter: u8) -> Vec<Cell> {
    let mut result = Vec::new();
    visit_fields(record, delimiter, |_, state, value| {
        result.push(Cell {
            state,
            value: value.into_owned(),
        });
        Ok(())
    })
    .expect("Field collection is infallible");
    result
}
#[derive(Clone, Copy)]
enum Kind {
    Unknown,
    Text,
    Float,
    Signed,
    Unsigned,
    Boolean,
}
fn number(value: &[u8]) -> Option<f64> {
    let parsed: f64 = std::str::from_utf8(value).ok()?.parse().ok()?;
    parsed.is_finite().then_some(parsed)
}
fn boolean(value: &[u8]) -> Option<bool> {
    let value = value.trim_ascii();
    if value.eq_ignore_ascii_case(b"true") {
        Some(true)
    } else if value.eq_ignore_ascii_case(b"false") {
        Some(false)
    } else {
        None
    }
}
fn typed(value: &[u8], kind: Kind, output: &mut Vec<u8>) -> Option<()> {
    match kind {
        Kind::Float => output.extend_from_slice(&number(value)?.to_le_bytes()),
        Kind::Signed => {
            let value: i64 = std::str::from_utf8(value).ok()?.parse().ok()?;
            output.extend_from_slice(&value.to_le_bytes());
        }
        Kind::Unsigned => {
            let value: u64 = std::str::from_utf8(value).ok()?.parse().ok()?;
            output.extend_from_slice(&value.to_le_bytes());
        }
        Kind::Boolean => output.push(u8::from(boolean(value)?)),
        _ => return None,
    }
    Some(())
}
fn u32_bytes(output: &mut Vec<u8>, value: usize) -> io::Result<()> {
    output.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| invalid("Columnar block is too large"))?
            .to_le_bytes(),
    );
    Ok(())
}
fn text(output: &mut Vec<u8>, value: &[u8]) -> io::Result<()> {
    u32_bytes(output, value.len())?;
    output.extend_from_slice(value);
    Ok(())
}

fn encode<R: Read, W: Write + Send>(
    input: R,
    output: &mut W,
    delimiter: u8,
    caps: &Capabilities,
    markers: &[String],
) -> io::Result<()> {
    let mut records = Records {
        input: BufReader::with_capacity(65536, input),
        line: 1,
        maximum: caps.maximum_record_bytes,
        scratch: Vec::new(),
    };
    let (_, mut header) = records
        .next()?
        .ok_or_else(|| invalid("No headers found on first line of input"))?;
    if header.starts_with(&[0xef, 0xbb, 0xbf]) {
        header.drain(..3);
    }
    let names: Vec<String> = fields(&header, delimiter)
        .into_iter()
        .filter(|cell| cell.state != 0)
        .map(|cell| {
            let name = std::str::from_utf8(&cell.value)
                .map_err(|_| invalid("Header names must be UTF-8"))?;
            Ok(name.trim_matches('"').trim_matches('\'').trim().to_owned())
        })
        .collect::<io::Result<_>>()?;
    if names.is_empty() || names.len() > 8192 {
        return Err(invalid("Invalid column count"));
    }
    let declarations: HashMap<&str, &str> = caps
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column.r#type.as_str()))
        .collect();
    let mut kinds = Vec::with_capacity(names.len());
    let mut encoded_header = Vec::new();
    u32_bytes(&mut encoded_header, names.len())?;
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        if name.is_empty()
            || name.len() > 255
            || name.chars().count() > 255
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || "_.-".contains(c))
            || !seen.insert(name)
            || matches!(name.as_str(), "timestamp" | "__COLLECTION_TAG")
        {
            return Err(invalid(format!(
                "Invalid, duplicate or reserved header '{name}'"
            )));
        }
        let declaration = declarations.get(name.as_str()).copied().unwrap_or("");
        let kind = match declaration {
            "Float64" => Kind::Float,
            "Int64" => Kind::Signed,
            "UInt64" => Kind::Unsigned,
            "Boolean" => Kind::Boolean,
            "" => Kind::Unknown,
            _ => Kind::Text,
        };
        kinds.push(kind);
        encoded_header.extend_from_slice(
            &u16::try_from(name.len())
                .map_err(|_| invalid("Header name is too long"))?
                .to_le_bytes(),
        );
        encoded_header.extend_from_slice(name.as_bytes());
        encoded_header
            .push(u8::try_from(declaration.len()).map_err(|_| invalid("Invalid declared type"))?);
        encoded_header.extend_from_slice(declaration.as_bytes());
    }
    if encoded_header.len() > caps.maximum_record_bytes {
        return Err(invalid("Header exceeds server memory limit"));
    }
    output.write_all(b"LSCB001\n")?;
    output.write_all(&(encoded_header.len() as u32).to_le_bytes())?;
    output.write_all(&encoded_header)?;
    output.flush()?;
    records.recycle(header);
    let row_capacity = caps.maximum_batch_rows.min(
        (caps.maximum_batch_bytes / (names.len() * std::mem::size_of::<CellRef>()).max(1)).max(1),
    );
    std::thread::scope(|scope| -> io::Result<()> {
        let (work_tx, work_rx) = std::sync::mpsc::sync_channel::<Batch>(1);
        let (free_tx, free_rx) = std::sync::mpsc::sync_channel::<Batch>(2);
        for _ in 0..2 {
            free_tx
                .send(Batch::new(names.len(), row_capacity))
                .map_err(|_| invalid("Cannot initialize frame buffers"))?;
        }
        let writer = &mut *output;
        let worker = std::thread::Builder::new()
            .name("logsh-column-writer".into())
            .spawn_scoped(scope, move || -> io::Result<()> {
                let mut payload = Vec::new();
                while let Ok(mut batch) = work_rx.recv() {
                    write_frame(
                        writer,
                        &batch,
                        &mut kinds,
                        caps.invariant_number_inference,
                        &mut payload,
                    )?;
                    batch.clear();
                    let _ = free_tx.send(batch);
                }
                Ok(())
            })?;
        let production = (|| -> io::Result<()> {
            let mut batch = free_rx
                .recv()
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Column writer stopped"))?;
            while let Some((line, record)) = records.next()? {
                let row_size = record.len().saturating_add(names.len() * 8 + 4);
                if !batch.lines.is_empty()
                    && (batch.lines.len() == row_capacity
                        || batch.size.saturating_add(row_size) > caps.maximum_batch_bytes)
                {
                    work_tx.send(batch).map_err(|_| {
                        io::Error::new(io::ErrorKind::BrokenPipe, "Column writer stopped")
                    })?;
                    batch = free_rx.recv().map_err(|_| {
                        io::Error::new(io::ErrorKind::BrokenPipe, "Column writer stopped")
                    })?;
                }
                if batch.size.saturating_add(row_size) > caps.maximum_batch_bytes {
                    return Err(invalid(format!(
                        "Record on line {line} exceeds the binary frame limit; use --encoding raw"
                    )));
                }
                batch.push(line, &record, delimiter, markers)?;
                records.recycle(record);
                batch.size += row_size;
            }
            if !batch.lines.is_empty() {
                work_tx.send(batch).map_err(|_| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "Column writer stopped")
                })?;
            }
            Ok(())
        })();
        drop(work_tx);
        drop(free_rx);
        let writing = worker
            .join()
            .map_err(|_| io::Error::other("Column writer panicked"))?;
        match (production, writing) {
            (Err(error), Err(writing)) if error.kind() == io::ErrorKind::BrokenPipe => Err(writing),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
            _ => Ok(()),
        }
    })?;
    output.write_all(&[0; 8])?;
    Ok(())
}
struct CellRef {
    state: u8,
    start: usize,
    length: usize,
}
#[derive(Clone, Copy)]
enum FirstValue {
    Unset,
    Candidate { start: usize, length: usize },
    Mixed,
}
struct Batch {
    lines: Vec<u64>,
    columns: Vec<Vec<CellRef>>,
    arena: Vec<u8>,
    first_values: Vec<FirstValue>,
    size: usize,
}
impl Batch {
    fn new(width: usize, capacity: usize) -> Self {
        Self {
            lines: Vec::with_capacity(capacity),
            columns: (0..width).map(|_| Vec::with_capacity(capacity)).collect(),
            arena: Vec::new(),
            first_values: vec![FirstValue::Unset; width],
            size: width * 10 + 8,
        }
    }
    fn clear(&mut self) {
        self.lines.clear();
        self.arena.clear();
        self.first_values.fill(FirstValue::Unset);
        self.size = self.columns.len() * 10 + 8;
        for column in &mut self.columns {
            column.clear();
        }
    }
    fn push(
        &mut self,
        line: u64,
        record: &[u8],
        delimiter: u8,
        markers: &[String],
    ) -> io::Result<()> {
        let width = self.columns.len();
        let count = visit_fields(record, delimiter, |column, mut state, value| {
            if column >= width {
                return Err(invalid(format!(
                    "Invalid number of columns on line {line}. Expected {width}."
                )));
            }
            if markers
                .iter()
                .any(|marker| marker.as_bytes() == value.as_ref())
            {
                state = 3;
            }
            let length = if state == 2 { value.len() } else { 0 };
            let start = if state == 2 {
                match self.first_values[column] {
                    FirstValue::Candidate { start, length }
                        if self.arena[start..start + length] == *value =>
                    {
                        start
                    }
                    previous => {
                        let start = self.arena.len();
                        self.arena.extend_from_slice(&value);
                        self.first_values[column] = if matches!(previous, FirstValue::Unset) {
                            FirstValue::Candidate { start, length }
                        } else {
                            FirstValue::Mixed
                        };
                        start
                    }
                }
            } else {
                self.arena.len()
            };
            self.columns[column].push(CellRef {
                state,
                start,
                length,
            });
            Ok(())
        })?;
        if count != width {
            return Err(invalid(format!(
                "Invalid number of columns on line {line}. Expected {width}, received {count}."
            )));
        }
        self.lines.push(line);
        Ok(())
    }
    fn value(&self, cell: &CellRef) -> &[u8] {
        &self.arena[cell.start..cell.start + cell.length]
    }
}

fn write_frame<W: Write>(
    output: &mut W,
    batch: &Batch,
    kinds: &mut [Kind],
    invariant: bool,
    payload: &mut Vec<u8>,
) -> io::Result<()> {
    payload.clear();
    payload.extend_from_slice(&batch.lines[0].to_le_bytes());
    for pair in batch.lines.windows(2) {
        u32_bytes(
            payload,
            usize::try_from(pair[1] - pair[0])
                .map_err(|_| invalid("Source line delta overflow"))?,
        )?;
    }
    let mut states = Vec::new();
    let mut data = Vec::new();
    let mut dictionary = Vec::new();
    let mut ids = Vec::new();
    let mut lookup: HashMap<&[u8], usize> = HashMap::new();
    for (cells, kind) in batch.columns.iter().zip(kinds.iter_mut()) {
        states.clear();
        states.resize(batch.lines.len().div_ceil(4), 0);
        data.clear();
        let mut raw_size: usize = 0;
        let mut value_count: usize = 0;
        for (row, cell) in cells.iter().enumerate() {
            states[row / 4] |= cell.state << (row % 4 * 2);
            if cell.state == 2 {
                raw_size += cell.length + 4;
                value_count += 1;
            }
            if matches!(kind, Kind::Unknown) && cell.state != 0 && cell.state != 3 {
                let value = batch.value(cell);
                *kind = if cell.state == 2 && invariant && number(value).is_some() {
                    Kind::Float
                } else if cell.state == 2 && boolean(value).is_some() {
                    Kind::Boolean
                } else {
                    Kind::Text
                };
            }
        }
        let constant = cells
            .iter()
            .find(|cell| cell.state == 2)
            .map(|cell| batch.value(cell))
            .filter(|first| {
                cells
                    .iter()
                    .filter(|cell| cell.state == 2)
                    .all(|cell| batch.value(cell) == *first)
            });
        let typed_width = match kind {
            Kind::Boolean => 1,
            Kind::Float | Kind::Signed | Kind::Unsigned => 8,
            _ => 0,
        };
        // Typed lanes avoid backend text conversion. Gzip removes repeated scalar bytes;
        // uncompressed size alone is a poor proxy for ingestion throughput.
        let is_typed = typed_width > 0
            && if let Some(value) = constant {
                if typed(value, *kind, &mut data).is_some() {
                    data.resize(value_count * typed_width, 0);
                    for written in (typed_width..data.len()).step_by(typed_width) {
                        data.copy_within(..typed_width, written);
                    }
                    true
                } else {
                    false
                }
            } else {
                cells
                    .iter()
                    .filter(|cell| cell.state == 2)
                    .all(|cell| typed(batch.value(cell), *kind, &mut data).is_some())
            };
        let encoding;
        if is_typed {
            encoding = match kind {
                Kind::Float => 2,
                Kind::Signed => 3,
                Kind::Unsigned => 4,
                Kind::Boolean => 5,
                _ => unreachable!(),
            };
        } else if let Some(value) =
            constant.filter(|value| value.len() + 8 + value_count * 4 < raw_size)
        {
            encoding = 1;
            data.clear();
            u32_bytes(&mut data, 1)?;
            text(&mut data, value)?;
            data.resize(data.len() + value_count * 4, 0);
        } else {
            dictionary.clear();
            ids.clear();
            lookup.clear();
            for cell in cells.iter().filter(|cell| cell.state == 2) {
                let value = batch.value(cell);
                let id = if let Some(id) = lookup.get(value) {
                    *id
                } else {
                    let id = lookup.len();
                    lookup.insert(value, id);
                    text(&mut dictionary, value)?;
                    id
                };
                u32_bytes(&mut ids, id)?;
            }
            data.clear();
            if dictionary.len() + ids.len() + 4 < raw_size {
                encoding = 1;
                u32_bytes(&mut data, lookup.len())?;
                data.extend_from_slice(&dictionary);
                data.extend_from_slice(&ids);
            } else {
                encoding = 0;
                for cell in cells.iter().filter(|cell| cell.state == 2) {
                    text(&mut data, batch.value(cell))?;
                }
            }
        }
        payload.push(encoding);
        u32_bytes(payload, states.len() + data.len())?;
        payload.extend_from_slice(&states);
        payload.extend_from_slice(&data);
    }
    output.write_all(&(batch.lines.len() as u32).to_le_bytes())?;
    output.write_all(&(payload.len() as u32).to_le_bytes())?;
    output.write_all(payload)?;
    Ok(())
}

#[cfg(test)]
fn frame<W: Write>(
    output: &mut W,
    rows: &[(u64, Vec<Cell>)],
    kinds: &mut [Kind],
    invariant: bool,
) -> io::Result<()> {
    let mut batch = Batch::new(kinds.len(), rows.len());
    for (line, cells) in rows {
        batch.lines.push(*line);
        for (column, cell) in cells.iter().enumerate() {
            let start = batch.arena.len();
            batch.arena.extend_from_slice(&cell.value);
            batch.columns[column].push(CellRef {
                state: cell.state,
                start,
                length: cell.value.len(),
            });
        }
    }
    write_frame(output, &batch, kinds, invariant, &mut Vec::new())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_lanes_preserve_inference_culture_and_prioritize_declared_conversion() {
        for (declaration, typed_codec) in [("", 2), ("Float64", 2), ("Int64", 3), ("UInt64", 4)] {
            for compatible in [false, true] {
                let caps = Capabilities {
                    version: 1,
                    maximum_record_bytes: 1 << 20,
                    maximum_batch_bytes: 1 << 20,
                    maximum_batch_rows: 1024,
                    invariant_number_inference: compatible,
                    columns: if declaration.is_empty() {
                        vec![]
                    } else {
                        vec![DeclaredColumn {
                            name: "value".into(),
                            r#type: declaration.into(),
                        }]
                    },
                };
                let mut output = Vec::new();
                encode(&b"value\n1\n2\n"[..], &mut output, b',', &caps, &[]).unwrap();
                let header = u32::from_le_bytes(output[8..12].try_into().unwrap()) as usize;
                let codec = output[12 + header + 8 + 8 + 4];
                if compatible || !declaration.is_empty() {
                    assert_eq!(codec, typed_codec);
                } else {
                    assert!(
                        codec < 2,
                        "Culture-sensitive inference must stay on the backend"
                    );
                }
            }
        }
    }
    #[test]
    fn repeated_fields_share_frame_storage_without_changing_later_or_recycled_values() {
        let mut batch = Batch::new(2, 4);
        for (line, record) in [
            b"same,one".as_slice(),
            b"same,two",
            b"same,three",
            b"other,one",
        ]
        .iter()
        .enumerate()
        {
            batch.push(line as u64 + 2, record, b',', &[]).unwrap();
        }
        let expected = [
            ["same", "same", "same", "other"],
            ["one", "two", "three", "one"],
        ];
        for (column, expected) in batch.columns.iter().zip(expected) {
            for (cell, expected) in column.iter().zip(expected) {
                assert_eq!(batch.value(cell), expected.as_bytes());
            }
        }
        assert!(batch.arena.len() < 4 * 4 + 5 + 3 + 3 + 3 + 5);
        batch.clear();
        batch.push(2, b"new,frame", b',', &[]).unwrap();
        assert_eq!(batch.value(&batch.columns[0][0]), b"new");
        assert_eq!(batch.value(&batch.columns[1][0]), b"frame");
    }
    #[test]
    fn terminal_chunk_and_completion_are_published_atomically() {
        let transfer = Arc::new(Transfer::default());
        let mut writer = QueueWriter {
            transfer: Arc::clone(&transfer),
            buffer: Vec::new(),
        };
        writer.write_all(b"tail").unwrap();
        writer.finish().unwrap();
        assert!(transfer.state.lock().unwrap().finished);
        assert!(!transfer.finished());
        let mut body = UploadBody {
            transfer: Arc::clone(&transfer),
        };
        let mut context = Context::from_waker(Waker::noop());
        let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut context) else {
            panic!("Missing terminal data");
        };
        assert_eq!(frame.into_data().unwrap().as_ref(), b"tail");
        assert!(transfer.finished());
        drop(body);
        assert!(transfer.finished());
    }
    #[test]
    fn early_response_cannot_acknowledge_queued_unconsumed_data() {
        let transfer = Arc::new(Transfer::default());
        transfer
            .push(Bytes::from_static(b"not consumed"), true)
            .unwrap();
        drop(UploadBody {
            transfer: Arc::clone(&transfer),
        });
        assert!(!transfer.finished());
    }
    #[test]
    fn tokenizer_preserves_empty_quoting_multiline_and_physical_lines() {
        let mut records = Records {
            input: BufReader::new(&b"a,b\r\n\"hello\r\n\"\"world\"\"\",\"\"\r\nx,\n"[..]),
            line: 1,
            maximum: 4096,
            scratch: Vec::new(),
        };
        assert_eq!(records.next().unwrap().unwrap().0, 1);
        let (line, raw) = records.next().unwrap().unwrap();
        assert_eq!(line, 2);
        let cells = fields(&raw, b',');
        assert_eq!(cells[0].value, b"hello\r\n\"world\"");
        assert_eq!(cells[1].state, 1);
        let (line, raw) = records.next().unwrap().unwrap();
        assert_eq!(line, 4);
        assert_eq!(fields(&raw, b',')[1].state, 0);
        assert!(records.next().unwrap().is_none());
    }
    #[test]
    fn quoted_empty_prevents_later_numeric_inference() {
        let rows = vec![
            (
                2,
                vec![Cell {
                    state: 1,
                    value: vec![],
                }],
            ),
            (
                3,
                vec![Cell {
                    state: 2,
                    value: b"123".to_vec(),
                }],
            ),
        ];
        let mut kinds = [Kind::Unknown];
        let mut bytes = Vec::new();
        frame(&mut bytes, &rows, &mut kinds, true).unwrap();
        assert!(matches!(kinds[0], Kind::Text));
    }
    #[test]
    fn malformed_and_oversized_records_fail_with_locations() {
        let mut records = Records {
            input: BufReader::new(&b"\"hello\nworld"[..]),
            line: 2,
            maximum: 4096,
            scratch: Vec::new(),
        };
        assert!(records.next().unwrap_err().to_string().contains("line 2"));
        let mut records = Records {
            input: BufReader::new(&b"123456\n"[..]),
            line: 4,
            maximum: 4,
            scratch: Vec::new(),
        };
        assert!(records.next().unwrap_err().to_string().contains("line 4"));
    }
}
