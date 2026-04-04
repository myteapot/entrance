use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context as AnyhowContext, Result};
use serde_json::{Map, Value};
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_subscriber::{
    fmt::MakeWriter,
    layer::{Context, Layer, SubscriberExt},
    util::SubscriberInitExt,
    EnvFilter,
};

use super::data_store::DataStore;

#[derive(Debug, Clone)]
pub struct LoggingSystem {
    log_file_path: Arc<PathBuf>,
    db_logging_enabled: bool,
}

impl LoggingSystem {
    pub fn init(
        log_dir: impl AsRef<Path>,
        log_level: &str,
        data_store: Option<DataStore>,
    ) -> Result<Self> {
        let log_dir = log_dir.as_ref();
        fs::create_dir_all(log_dir)
            .with_context(|| format!("failed to create log directory at {}", log_dir.display()))?;

        let log_file_path = log_dir.join("entrance.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .with_context(|| format!("failed to open log file at {}", log_file_path.display()))?;

        let filter = EnvFilter::try_new(log_level)
            .with_context(|| format!("invalid tracing filter `{log_level}`"))?;
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_writer(SharedFileMakeWriter::new(file));

        let db_logging_enabled = data_store.is_some();

        match data_store {
            Some(data_store) => tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .with(DatabaseLogLayer::new(data_store))
                .try_init()?,
            None => tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .try_init()?,
        }

        tracing::info!(
            log_file = %log_file_path.display(),
            db_logging_enabled,
            "logging initialized"
        );

        Ok(Self {
            log_file_path: Arc::new(log_file_path),
            db_logging_enabled,
        })
    }

    pub fn log_file_path(&self) -> &Path {
        self.log_file_path.as_ref().as_path()
    }

    pub fn db_logging_enabled(&self) -> bool {
        self.db_logging_enabled
    }
}

#[derive(Clone)]
struct SharedFileMakeWriter {
    file: Arc<Mutex<File>>,
}

impl SharedFileMakeWriter {
    fn new(file: File) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
        }
    }
}

impl<'a> MakeWriter<'a> for SharedFileMakeWriter {
    type Writer = SharedFileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFileWriter {
            file: Arc::clone(&self.file),
        }
    }
}

struct SharedFileWriter {
    file: Arc<Mutex<File>>,
}

impl Write for SharedFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file lock poisoned"))?;
        file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file lock poisoned"))?;
        file.flush()
    }
}

#[derive(Clone)]
struct DatabaseLogLayer {
    data_store: DataStore,
}

impl DatabaseLogLayer {
    fn new(data_store: DataStore) -> Self {
        Self { data_store }
    }
}

impl<S> Layer<S> for DatabaseLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let payload = serde_json::json!({
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "name": metadata.name(),
            "fields": visitor.fields,
        });
        let payload = payload.to_string();

        let topic = metadata.target();
        let _ = self.data_store.append_core_event_log(topic, Some(&payload));
    }
}

#[derive(Default)]
struct JsonVisitor {
    fields: Map<String, Value>,
}

impl Visit for JsonVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(number) = serde_json::Number::from_f64(value) {
            self.fields
                .insert(field.name().to_string(), Value::Number(number));
        }
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{value:?}")),
        );
    }
}
