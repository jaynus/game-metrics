#![allow(unused_variables, dead_code)]
#[cfg(feature = "threads")]
use crossbeam_channel::{Receiver, Sender};
#[cfg(feature = "threads")]
use parking_lot::Mutex;
#[cfg(feature = "threads")]
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
#[cfg(not(feature = "threads"))]
use std::{
    cell::RefCell,
    collections::VecDeque
};

use fxhash::FxHashMap;
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

#[cfg(feature = "threads")]
lazy_static::lazy_static! {
    static ref LOGGER: Arc<Logger> = Arc::new(Logger::default());
}

#[cfg(not(feature = "threads"))]
lazy_static::lazy_static! {
    static ref LOGGER: Logger = Logger::default();
}

struct InternalRecord {
    target: String,
    level: log::Level,
    message: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

pub struct Settings {
    pub targets: FxHashMap<String, Level>,
    paths: Vec<String>,
    #[cfg(not(feature = "threads"))]
    autoflush: bool,
}
#[cfg(feature = "threads")]
impl Default for Settings {
    fn default() -> Self {
        Self {
            targets: FxHashMap::default(),
            paths: Vec::default(),
        }
    }
}
#[cfg(not(feature = "threads"))]
impl Default for Settings {
    fn default() -> Self {
        Self {
            targets: FxHashMap::default(),
            paths: Vec::default(),
            autoflush: true,
        }
    }
}

#[cfg(not(feature = "threads"))]
unsafe impl Send for Settings {}

#[cfg(not(feature = "threads"))]
unsafe impl Sync for Settings {}

#[cfg(feature = "threads")]
pub struct Logger {
    worker_handle: Option<JoinHandle<()>>,
    worker_flag: Arc<AtomicBool>,
    settings: Arc<Mutex<Settings>>,
    channel: (Sender<InternalRecord>, Receiver<InternalRecord>),
}
#[cfg(feature = "threads")]
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
        *LOGGER.borrow_mut().settings = settings;
    }

    pub fn init() -> Result<(), SetLoggerError> {
        log::set_logger(&*LOGGER.as_ref()).map(|()| log::set_max_level(LevelFilter::Trace))
    }
}
#[cfg(feature = "threads")]
impl Log for Logger {
    fn enabled(&self, meta: &Metadata) -> bool {
        self.settings
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

#[cfg(feature = "threads")]
impl Drop for Logger {
    fn drop(&mut self) {
        self.worker_flag.store(false, Ordering::Relaxed);
        self.worker_handle.take().unwrap().join().unwrap();
    }
}

#[cfg(not(feature = "threads"))]
pub struct Logger {
    queue: RefCell<VecDeque<InternalRecord>>,
    settings: Settings,
}
#[cfg(not(feature = "threads"))]
impl Logger {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            queue: RefCell::new(VecDeque::with_capacity(1024)),
        }
    }


    pub fn init() -> Result<(), SetLoggerError> {
        log::set_logger(&*LOGGER).map(|()| log::set_max_level(LevelFilter::Trace))
    }
}
#[cfg(not(feature = "threads"))]
impl Log for Logger {
    fn enabled(&self, meta: &Metadata) -> bool {
        self.settings
            .targets
            .get(meta.target())
            .map(|level| *level <= meta.level())
            .unwrap_or(false)
    }

    fn log(&self, record: &Record) {
        let event = InternalRecord {
            target: record.target().to_owned(),
            level: record.level(),
            message: format!("{}", record.args()),
            module_path: record.module_path().map(|s| s.to_owned()),
            file: record.file().map(|s| s.to_owned()),
            line: record.line(),
        };
        if self.enabled(record.metadata()) && !self.settings.autoflush {
            self.queue.borrow_mut().push_back(event);
        } else {
            println!("{}", event.message.to_string());
        }
    }

    fn flush(&self) {
        let mut queue = self.queue.borrow_mut();
        while let Some(event) = queue.pop_front() {
            println!("{}", event.message.to_string());
        }
    }
}

#[cfg(not(feature = "threads"))]
unsafe impl Send for Logger {}

#[cfg(not(feature = "threads"))]
unsafe impl Sync for Logger {}

impl Default for Logger {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

