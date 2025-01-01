// Will create an exporter with a single metric that does not change

use env_logger::{
    Builder,
    Env,
};
use log::{debug, info, trace};
use prometheus_exporter::prometheus::{
    register_gauge, register_gauge_vec
};
use std::net::SocketAddr;

fn get_addr() -> SocketAddr {
    let host = std::env::var("OWNTRACKS_EXPORTER_BIND_HOST")
        .unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("OWNTRACKS_EXPORTER_BIND_PORT")
        .unwrap_or("9192".to_string());
    let addr_str = format!("{host}:{port}");
    addr_str
        .parse()
        .expect(format!("This doesn't seem to be a valid bind address: {addr_str}").as_str())
}

fn main() {
    // Setup logger with default level info so we can see the messages from
    // prometheus_exporter.
    Builder::from_env(Env::default().default_filter_or("info")).init();

    // Parse address used to bind exporter to.
    let addr = get_addr();

    // First self-request to make sure the server started correctly
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    {
      let barrier = barrier.clone();
      std::thread::spawn(move || {
        trace!("waiting on client barrier");
        barrier.wait();
        let body = reqwest::blocking::get(format!("http://{addr}")).unwrap().text().unwrap();
        info!("initial metrics look fine:\n{body}");
      });
    }

    // Create metric
    let metric = register_gauge!("simple_the_answer", "to everything")
        .expect("can not create gauge simple_the_answer");
    metric.set(42.0);

    let metrics = register_gauge_vec!("simple_the_answers", "to many things", &["a", "b"])
        .expect("can not create gauge simple_the_answers");
    metrics.with_label_values(&vec!["A", "B"]).set(42.0);

    // Start exporter
    let exporter = prometheus_exporter::start(addr).expect("can not start exporter");

    barrier.wait();

    loop {
        debug!("waiting for next request...");
        let guard = exporter.wait_request();
        debug!("updating metrics...");
        metric.inc();
        metrics.with_label_values(&vec!["A", "B"]).inc();
        metrics.with_label_values(&vec!["C", "D"]).inc();
        debug!("updating metrics...");
        drop(guard);
    }
}