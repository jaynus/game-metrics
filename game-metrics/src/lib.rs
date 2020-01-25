use crossbeam_channel::{Receiver, Sender};
use fxhash::FxHashMap;
pub use hdrhistogram as histogram;
use hdrhistogram::Histogram;
use parking_lot::Mutex;
use quanta::Clock;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

#[macro_export]
pub use game_metrics_macro::instrument;

#[macro_export]
macro_rules! scope(
    ($span_name:literal) => (
        let __INSTR_METRICS_SCOPE = $crate::Span::new($span_name);
    )
);

pub struct Channel {
    channel: (Sender<Event>, Receiver<Event>),
    clock: Clock,
}
impl Channel {
    fn new() -> Self {
        Self {
            channel: crossbeam_channel::unbounded(),
            clock: Clock::default(),
        }
    }
    pub fn send(&self, event: Event) {
        self.channel.0.send(event);
    }

    pub fn recv(&self) -> Option<Event> {
        if let Ok(event) = self.channel.1.try_recv() {
            Some(event)
        } else {
            None
        }
    }
}

pub enum Event {
    Enter(&'static str),
    Exit {
        span_name: &'static str,
        elapsed: u64,
    },
}
impl Event {
    fn name(&self) -> &'static str {
        match self {
            Event::Enter(name) => name,
            Event::Exit { span_name, .. } => span_name,
        }
    }
}

lazy_static::lazy_static! {
    static ref CHANNEL: Channel = Channel::new();
}

pub struct Span {
    name: &'static str,
    start: u64,
}
impl Span {
    pub fn new(name: &'static str) -> Self {
        CHANNEL.send(Event::Enter(name));

        Self {
            name,
            start: CHANNEL.clock.now(),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let elapsed = CHANNEL.clock.now() - self.start;
        CHANNEL.send(Event::Exit {
            span_name: self.name,
            elapsed,
        });
    }
}

pub struct Metrics {
    histograms: Arc<Mutex<FxHashMap<&'static str, Histogram<u64>>>>,

    worker_handle: Option<JoinHandle<()>>,
    worker_flag: Arc<AtomicBool>,
}

impl Metrics {
    pub fn for_each_histogram<F>(&self, f: F)
    where
        F: Fn(&'static str, &Histogram<u64>),
    {
        self.histograms
            .lock()
            .iter()
            .for_each(|(name, histogram)| (f)(name, histogram))
    }

    pub fn new(sigfig: u8) -> Metrics {
        let worker_flag = Arc::new(AtomicBool::new(true));
        let histograms = Arc::new(Mutex::new(FxHashMap::default()));

        let inner_histograms = histograms.clone();
        let inner_flag = worker_flag.clone();
        let worker_handle = std::thread::spawn(move || {
            while inner_flag.load(Ordering::Relaxed) {
                while let Some(event) = CHANNEL.recv() {
                    match event {
                        Event::Exit { span_name, elapsed } => {
                            inner_histograms
                                .lock()
                                .entry(span_name)
                                .or_insert(
                                    Histogram::new_with_bounds(1, 1_000_000_000, sigfig).unwrap(),
                                )
                                .record(elapsed);
                        }
                        _ => {}
                    }
                }
            }
        });

        Self {
            histograms,
            worker_flag,
            worker_handle: Some(worker_handle),
        }
    }
}
impl Drop for Metrics {
    fn drop(&mut self) {
        self.worker_flag.store(false, Ordering::Relaxed);
        self.worker_handle.take().unwrap().join().unwrap();
    }
}
