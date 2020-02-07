use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::{
    thread::JoinHandle,
    sync::{Arc, atomic::{Ordering, AtomicBool}},
    cell::RefCell,
};
use crossbeam_channel::{Sender, Receiver};

lazy_static::lazy_static! {
    static ref LOGGER: Arc<Logger> = Arc::new(Logger::new());
}

struct InternalRecord {
    target: String,
    level: log::Level,
    message: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

pub struct LoggerSettings {

}

pub struct Logger {
    worker_handle: Option<JoinHandle<()>>,
    worker_flag: Arc<AtomicBool>,
    channel: (Sender<InternalRecord>, Receiver<InternalRecord>),

}
impl Logger {
    pub fn new() -> Self {
        let worker_flag = Arc::new(AtomicBool::new(true));

        let channel: (Sender<InternalRecord>, Receiver<InternalRecord>) = crossbeam_channel::bounded(4096);

        let inner_flag = worker_flag.clone();
        let receiver = channel.1.clone();
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
            channel,
        }
    }
    pub fn init() -> Result<(), SetLoggerError> {
        log::set_logger(&*LOGGER.as_ref()).map(|()| log::set_max_level(LevelFilter::Trace))
    }
}
impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.channel.0.send(InternalRecord {
                target: record.target().to_owned(),
                level: record.level(),
                message: format!("{}", record.args()),
                module_path: record.module_path().map(|s| s.to_owned()),
                file: record.file().map(|s| s.to_owned()),
                line: record.line()
            });
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