#![allow(dead_code)]

//!
//!
//! # Examples
//! ```
//! use game_metrics::{instrument, Metrics};
//!
//! #[instrument]
//! fn scoped() {
//!     let do_stuff = 1 + 1;
//! }
//!
//! #[instrument(name = "test")]
//! fn named() {
//!    let do_stuff = 1 + 1;
//! }
//!
//! let metrics = Metrics::new(1);
//!
//! (0..1000).for_each(|_| scoped());
//! (0..1000).for_each(|_| named());
//!
//! let mut count = 0;
//!
//! metrics.flush();
//! metrics.for_each_histogram(|span_name, h| {
//!     println!("{}", span_name);
//!     assert!(h.mean() > 0.0);
//!     count += 1;
//! });
//! assert_eq!(count, 2);
//! ```
//!
//! ```
//! use game_metrics::{scope, Metrics};
//!
//! fn scoped() {
//!
//! }
//!
//! let metrics = Metrics::new(1);
//!
//! (0..1000).for_each(|_| scoped());
//!
//! metrics.flush();
//! metrics.for_each_histogram(|span_name, h| {
//!     assert_eq!(span_name, "MyScope");
//!     assert!(h.mean() > 0.0);
//! });
//! ```

#[cfg(feature = "threads")]
use crossbeam_channel::{Receiver, Sender};

#[cfg(not(feature = "threads"))]
use std::{
    cell::RefCell,
    collections::VecDeque
};

use fxhash::FxHashMap;
pub use hdrhistogram as histogram;
use hdrhistogram::Histogram;
use parking_lot::Mutex;
use quanta::Clock;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

lazy_static::lazy_static! {
    static ref CHANNEL: Channel = Channel::new();
}

#[cfg(not(feature = "disable"))]
#[macro_export]
macro_rules! scope(
    ($span_name:literal) => (
        let __INSTR_METRICS_SCOPE = $crate::Span::new($span_name);
    )
);

#[cfg(feature = "disable")]
#[macro_export]
macro_rules! scope(
    ($span_name:literal) => (

    )
);

/// Events dispatched by various instrumentation functions.
pub enum Event {
    /// A `Span` has been entered
    SpanEnter(&'static str),
    /// A `Span` has been dropped
    SpanExit {
        span_name: &'static str,
        elapsed: u64,
    },
}

#[cfg(feature = "threads")]
struct Channel {
    subscribers: AtomicU32,
    channel: (Sender<Event>, Receiver<Event>),
    clock: Clock,
}
#[cfg(feature = "threads")]
impl Channel {
    fn new() -> Self {
        Self {
            subscribers: AtomicU32::new(0),
            channel: crossbeam_channel::unbounded(),
            clock: Clock::default(),
        }
    }
    #[cfg(not(feature = "disable"))]
    fn send(&self, event: Event) {
        if self.subscribers.load(Ordering::SeqCst) > 0 {
            self.channel.0.send(event).unwrap();
        }
    }

    #[cfg(not(feature = "disable"))]
    fn recv(&self) -> Option<Event> {
        if self.subscribers.load(Ordering::SeqCst) > 0 {
            if let Ok(event) = self.channel.1.try_recv() {
                return Some(event);
            }
        }
        None
    }

    #[cfg(feature = "disable")]
    fn send(&self, event: Event) {}
    #[cfg(feature = "disable")]
    fn recv(&self) -> Option<Event> {
        None
    }
}

#[cfg(not(feature = "threads"))]
struct Channel {
    queue: RefCell<VecDeque<Event>>,
    clock: Clock,
}
#[cfg(not(feature = "threads"))]
impl Channel {
    fn new() -> Self {
        Self {
            queue: RefCell::new(VecDeque::with_capacity(1024)),
            clock: Clock::default(),
        }
    }

    #[cfg(not(feature = "disable"))]
    fn send(&self, event: Event) {
        self.queue.borrow_mut().push_back(event);
    }

    #[cfg(not(feature = "disable"))]
    fn recv(&self) -> Option<Event> {
        self.queue.borrow_mut().pop_front()
    }

    #[cfg(feature = "disable")]
    fn send(&self, event: Event) {}
    #[cfg(feature = "disable")]
    fn recv(&self) -> Option<Event> {
        None
    }
}

#[cfg(not(feature = "threads"))]
unsafe impl Send for Channel {}

#[cfg(not(feature = "threads"))]
unsafe impl Sync for Channel {}

/// The raw span RAII guard which is used for scoping spans of code for instrumentation.
///
/// On construction, the `Span` emits a `Event::SpanEnter` event, and saves its creation time.
///
/// On `Drop`, the `Span` emits an `Event::SpanExit` event, which includes the elapsed time
/// calculated from the saved start time to the time of drop.
pub struct Span {
    name: &'static str,
    start: u64,
}
impl Span {
    pub fn new(name: &'static str) -> Self {
        CHANNEL.send(Event::SpanEnter(name));

        Self {
            name,
            start: CHANNEL.clock.now(),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let elapsed = CHANNEL.clock.now() - self.start;
        CHANNEL.send(Event::SpanExit {
            span_name: self.name,
            elapsed,
        });
    }
}

/// The metrics struct is used to initialize the metrics communications channels and access the
/// `Histogram` data for each named metric.
pub struct Metrics {
    #[cfg(feature = "threads")]
    histograms: Arc<Mutex<FxHashMap<&'static str, Histogram<u64>>>>,
    #[cfg(feature = "threads")]
    worker_handle: Option<JoinHandle<()>>,
    #[cfg(feature = "threads")]
    worker_flag: Arc<AtomicBool>,

    #[cfg(not(feature = "threads"))]
    histograms: RefCell<FxHashMap<&'static str, Histogram<u64>>>,

    sigfig: u8,
}

impl Metrics {

    /// Iterate the histograms created. This function accepts a closure of `FnMut(&'static str, &Histogram<u64>)`
    /// taking the span name and the histogram as arguments.
    pub fn for_each_histogram<F>(&self, mut f: F)
    where
        F: FnMut(&'static str, &Histogram<u64>),
    {
        #[cfg(feature = "threads")]
        {
            self.histograms
                .lock()
                .iter()
                .for_each(|(name, histogram)| (f)(name, histogram))
        }

        #[cfg(not(feature = "threads"))]
        {
            self.flush();
            self.histograms.borrow()
                .iter()
                .for_each(|(name, histogram)| (f)(name, histogram))
        }
    }

    /// Blocks the current thread until the worker thread has completed flushing the receiver.
    ///
    /// # Warning
    /// In high contention situations, this may block indefinitely. This method is meant to be
    /// used in the context of a game engine, where execution can be guaranteed to be blocked for
    /// metric collection.
    #[cfg(feature = "threads")]
    pub fn flush(&self) {
        while !CHANNEL.channel.1.is_empty() {
            std::thread::yield_now();
        }
    }
    #[cfg(not(feature = "threads"))]
    pub fn flush(&self) {
        while let Some(event) = CHANNEL.recv() {
            if let Event::SpanExit { span_name, elapsed } = event {
                self.histograms
                    .borrow_mut()
                    .entry(span_name)
                    .or_insert_with(
                        || Histogram::new_with_bounds(1, 1_000_000_000, self.sigfig).unwrap(),
                    )
                    .record(elapsed);
            }
        }
    }

    /// Creates a new metrcs instance, initializing metrics and spawning a worker to collect the data.
    ///
    /// # Warning
    /// Any given instance of `Metrics` will globally collect a duplicate of the `Histgram` data. Only
    /// one instance should be active at a time.
    #[allow(unused_must_use)]
    #[cfg(feature = "threads")]
    pub fn new(sigfig: u8) -> Metrics {
        let worker_flag = Arc::new(AtomicBool::new(true));
        let histograms = Arc::new(Mutex::new(FxHashMap::default()));

        let inner_histograms = histograms.clone();
        let inner_flag = worker_flag.clone();
        let worker_handle = std::thread::spawn(move || {
            while inner_flag.load(Ordering::Relaxed) {
                while let Some(event) = CHANNEL.recv() {
                    if let Event::SpanExit { span_name, elapsed } = event {
                        inner_histograms
                            .lock()
                            .entry(span_name)
                            .or_insert_with(
                                || Histogram::new_with_bounds(1, 1_000_000_000, sigfig).unwrap(),
                            )
                            .record(elapsed);
                    }
                }
            }
        });

        CHANNEL.subscribers.fetch_add(1, Ordering::SeqCst);

        Self {
            histograms,
            worker_flag,
            sigfig,
            worker_handle: Some(worker_handle),
        }
    }

    #[cfg(not(feature = "threads"))]
    pub fn new(sigfig: u8) -> Metrics {
        Self {
            histograms: RefCell::new(FxHashMap::default()),
            sigfig
        }
    }
}
impl Drop for Metrics {
    fn drop(&mut self) {
        #[cfg(feature = "threads")]
            {
                CHANNEL.subscribers.fetch_sub(1, Ordering::SeqCst);

                self.worker_flag.store(false, Ordering::Relaxed);
                self.worker_handle.take().unwrap().join().unwrap();
            }
    }
}
