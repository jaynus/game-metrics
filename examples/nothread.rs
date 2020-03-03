use game_metrics::{instrument, scope, Metrics};
use std::time::Duration;

#[instrument]
fn long() {
    std::thread::sleep(Duration::from_millis(500));
}

#[instrument]
fn short() {
    std::thread::sleep(Duration::from_millis(50));
}

fn long_scoped() {
    scope!("long_scoped");
    std::thread::sleep(Duration::from_millis(500));
}

fn short_scoped() {
    scope!("short_scoped");
    std::thread::sleep(Duration::from_millis(50));
}

fn main() {
    let metrics = Metrics::new(1);

    (0..2).for_each(|_| long_scoped());
    (0..2).for_each(|_| long());
    (0..2).for_each(|_| short_scoped());
    (0..2).for_each(|_| short());

    (0..2).for_each(|_| long_scoped());
    (0..2).for_each(|_| long());
    (0..2).for_each(|_| short_scoped());
    (0..2).for_each(|_| short());

    metrics.for_each_histogram(|span_name, h| {
        println!("{} -> {:.2}ms", span_name, h.mean() / 1_000_000.0);
    });
}
