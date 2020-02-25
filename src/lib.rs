/// # Game-oriented Metrics Collection
///
/// This library leverages `hdrhistogram` and `crossbeam-channel` to allow collecting non-blocking
/// timing metrics. This library is geared towards the specific needs of measuring metrics in a game-specific
/// context; meaning span metrics and frame metrics; although many better libraries exist, these are
/// all mainly geared towards web, server and async contexts.
///
/// The `disable` feature exists to disable the system at compile time.
///
/// # Example
/// ```
/// use game_metrics::{scope, instrument, Metrics};
///use std::time::Duration;
///
///#[instrument]
///fn long() {
///    std::thread::sleep(Duration::from_millis(500));
///}
///
///#[instrument]
///fn short() {
///    std::thread::sleep(Duration::from_millis(50));
///}
///
///
///fn long_scoped() {
///    scope!("long_scoped");
///    std::thread::sleep(Duration::from_millis(500));
///}
///
///
///fn short_scoped() {
///    scope!("short_scoped");
///    std::thread::sleep(Duration::from_millis(50));
///}
///
///fn main() {
///    let metrics = Metrics::new(1);
///
///    let t1 =  std::thread::spawn(|| {
///        (0..10).for_each(|_| long_scoped());
///        (0..10).for_each(|_| long());
///        (0..10).for_each(|_| short_scoped());
///        (0..10).for_each(|_| short());
///    });
///
///    let t2 =  std::thread::spawn(|| {
///        (0..10).for_each(|_| long_scoped());
///        (0..10).for_each(|_| long());
///        (0..10).for_each(|_| short_scoped());
///        (0..10).for_each(|_| short());
///    });
///
///    t1.join().unwrap();
///    t2.join().unwrap();
///
///    metrics.for_each_histogram(|span_name, h| {
///        println!("{} -> {:.2}ms", span_name, h.mean() / 1_000_000.0);
///    });
///}
/// ```


#[cfg(feature = "metrics")]
mod metrics;

#[cfg(feature = "metrics")]
pub use metrics::{Event, Metrics, RawSubscription, Span};

#[cfg(feature = "metrics")]
pub use game_metrics_macro::instrument;

#[cfg(feature = "logging")]
mod logging;

#[cfg(feature = "logging")]
pub use logging::{Logger, Settings as LoggerSettings};

pub use log::Level as LogLevel;