#![allow(unused_variables, dead_code)]
use crossbeam_channel::{Receiver, Sender};
use fxhash::FxHashMap;
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use parking_lot::Mutex;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

lazy_static::lazy_static! {
    static ref LOGGER: Arc<Logger> = Arc::new(Logger::default());
}

struct InternalRecord {
    target: String,
    level: log::Level,
    message: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

#[derive(Default)]
pub struct Settings {
    targets: FxHashMap<String, Level>,
    _paths: Vec<String>,
}

pub struct Logger {
    worker_handle: Option<JoinHandle<()>>,
    worker_flag: Arc<AtomicBool>,
    settings: Arc<Mutex<Settings>>,
    channel: (Sender<InternalRecord>, Receiver<InternalRecord>),
}
impl Default for Logger {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}
impl Logger {
    pub fn new(settings: Settings) -> Self {
        let worker_flag = Arc::new(AtomicBool::new(true));

        let channel: (Sender<InternalRecord>, Receiver<InternalRecord>) =
            crossbeam_channel::bounded(4096);

        let settings = Arc::new(Mutex::new(settings));

        let inner_flag = worker_flag.clone();
        let receiver = channel.1.clone();
        let worker_settings = settings.clone();

        let worker_handle = std::thread::spawn(move || {
            while inner_flag.load(Ordering::Relaxed) {
                while let Ok(event) = receiver.recv() {
                    println!("{}", event.message.to_string());
                }
            }
        });

        Self {
            worker_handle: Some(worker_handle),
            worker_flag,
            settings,
            channel,
        }
    }

    pub fn change_settings(settings: Settings) {
        *LOGGER.as_ref().settings.lock() = settings;
    }

    pub fn init() -> Result<(), SetLoggerError> {
        log::set_logger(&*LOGGER.as_ref()).map(|()| log::set_max_level(LevelFilter::Trace))
    }
}
impl Log for Logger {
    fn enabled(&self, meta: &Metadata) -> bool {
        self.settings
            .lock()
            .targets
            .get(meta.target())
            .map(|level| *level <= meta.level())
            .unwrap_or(false)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.channel.0.send(InternalRecord {
                target: record.target().to_owned(),
                level: record.level(),
                message: format!("{}", record.args()),
                module_path: record.module_path().map(|s| s.to_owned()),
                file: record.file().map(|s| s.to_owned()),
                line: record.line(),
            }).unwrap_or_else(|e| println!("Failed to write channel: {:?}", e) );
        }
    }

    fn flush(&self) {}
}

impl Drop for Logger {
    fn drop(&mut self) {
        self.worker_flag.store(false, Ordering::Relaxed);
        self.worker_handle.take().unwrap().join().unwrap();
    }
}
