// Will create an exporter with a single metric that does not change

use env_logger::{Builder, Env};
use log::{debug, error, info, trace};
#[cfg(feature = "sample_metrics")]
use prometheus_exporter::prometheus::{register_gauge, register_gauge_vec};
use prometheus_exporter::prometheus::{register_int_counter_vec, IntCounterVec};
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    net::SocketAddr,
    ops::AddAssign,
    path::{Path, PathBuf},
};

fn get_addr() -> SocketAddr {
    let host = std::env::var("OWNTRACKS_EXPORTER_BIND_HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("OWNTRACKS_EXPORTER_BIND_PORT").unwrap_or("9192".to_string());
    let addr_str = format!("{host}:{port}");
    addr_str
        .parse()
        .unwrap_or_else(|_| panic!("This doesn't seem to be a valid bind address: {addr_str}"))
}

fn get_storage_dir() -> String {
    std::env::var("OWNTRACKS_EXPORTER_STORAGE_DIR").unwrap_or("/otr-storage".to_string())
}

struct StorageAccountant {
    root: String,
    m_points_total: IntCounterVec,
    m_lwts_total: IntCounterVec,
}

#[derive(Eq, Hash, PartialEq)]
struct StorageDevice {
    user_name: String,
    device_name: String,
}

#[derive(Default)]
struct StorageDeviceStats {
    points_count_total: usize,
    ltws_count_total: usize,
}

impl AddAssign for StorageDeviceStats {
    fn add_assign(&mut self, rhs: Self) {
        self.points_count_total += rhs.points_count_total;
        self.ltws_count_total += rhs.ltws_count_total;
    }
}

impl StorageAccountant {
    fn new(root: &str) -> Self {
        Self {
            root: root.to_owned(),
            m_points_total: register_int_counter_vec!(
                "owntracks_recorder_points_total",
                "Total number of points recorded so far",
                &["user", "device"]
            )
            .unwrap(),
            m_lwts_total: register_int_counter_vec!(
                "owntracks_recorder_lwts_total",
                "Total number of LWTs recorded so far",
                &["user", "device"]
            )
            .unwrap(),
        }
    }

    fn get_all_dir_entries(dir: &Path, filter: impl Fn(&PathBuf) -> bool) -> Vec<String> {
        let mut subdirs: Vec<String> = Vec::new();
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if filter(&path) {
                                if let Some(basename) = path.file_name() {
                                    if let Some(basename) = basename.to_str() {
                                        trace!(
                                            "found entry: {}/{}",
                                            dir.as_os_str().to_str().unwrap_or("?"),
                                            basename
                                        );
                                        subdirs.push(basename.to_owned())
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Error dir entry {}: {}", dir.display(), e),
                    }
                }
            }
            Err(e) => error!("Error read dir {}: {}", dir.display(), e),
        }
        subdirs
    }

    fn get_all_subdirs(dir: &Path) -> Vec<String> {
        Self::get_all_dir_entries(dir, |path| path.is_dir())
    }

    fn get_all_files(dir: &Path) -> Vec<String> {
        Self::get_all_dir_entries(dir, |path| path.is_file())
    }

    fn get_user_names(&self) -> Vec<String> {
        let last_dir = Path::new(&self.root).join("last");
        Self::get_all_subdirs(&last_dir)
    }

    fn get_device_names(&self, user: &str) -> Vec<String> {
        let user_dir = Path::new(&self.root).join("last").join(user);
        Self::get_all_subdirs(&user_dir)
    }

    fn get_devices(&self) -> Vec<StorageDevice> {
        let mut devices = Vec::new();
        for user_name in self.get_user_names() {
            for device_name in self.get_device_names(&user_name) {
                devices.push(StorageDevice {
                    user_name: user_name.clone(),
                    device_name,
                });
            }
        }
        devices
    }

    fn to_labels_map(device: &StorageDevice) -> HashMap<&str, &str> {
        HashMap::from([
            ("user", device.user_name.as_str()),
            ("device", device.device_name.as_str()),
        ])
    }

    fn get_rec_file_stats(
        &self,
        dir: &PathBuf,
        file: &str,
    ) -> Result<StorageDeviceStats, std::io::Error> {
        let rec_file_path = Path::new(dir).join(file);
        let file = File::open(rec_file_path)?;
        let r = BufReader::new(file);
        let lines = r.lines();
        let stats = lines
            .map(|line| {
                line.as_ref().map_or(String::new(), |line| {
                    line.split_whitespace()
                        .nth(1)
                        .map_or(String::new(), |_2nd_field| _2nd_field.to_string())
                })
            })
            .fold(StorageDeviceStats::default(), |mut stats, _2nd_field| {
                match _2nd_field.as_str() {
                    "*" => stats.points_count_total += 1,
                    "lwt" => stats.ltws_count_total += 1,
                    _ => (),
                }
                stats
            });
        Ok(stats)
    }

    fn get_all_locations_count(&self, device: &StorageDevice) -> StorageDeviceStats {
        let dir = Path::new(&self.root)
            .join("rec")
            .join(&device.user_name)
            .join(&device.device_name);
        let total = Self::get_all_files(&dir)
            .iter()
            .map(|file| self.get_rec_file_stats(&dir, file))
            .fold(StorageDeviceStats::default(), |mut acc, stat| {
                if let Ok(stat) = stat {
                    acc += stat;
                }
                acc
            });
        total
    }

    fn update(&mut self) {
        for device in self.get_devices() {
            let total = self.get_all_locations_count(&device);
            let labels = Self::to_labels_map(&device);
            let m_points_total = self.m_points_total.with(&labels);
            m_points_total.reset();
            m_points_total.inc_by(total.points_count_total as u64);
            let m_lwts_total = self.m_lwts_total.with(&labels);
            m_lwts_total.reset();
            m_lwts_total.inc_by(total.ltws_count_total as u64);
        }
    }
}

fn setup_signal_handling() {
    let mut signals = Signals::new(&[SIGINT, SIGTERM]).expect("Unable to register signals");

    // Spawn a thread to handle incoming signals
    std::thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGINT => {
                    println!("Caught SIGINT (Ctrl+C), shutting down...");
                    // Clean up or do necessary work before exiting
                    std::process::exit(0); // Exit the process cleanly
                }
                SIGTERM => {
                    println!("Caught SIGTERM, shutting down...");
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    });
}

fn main() {
    // Setup logger with default level info so we can see the messages from
    // prometheus_exporter.
    Builder::from_env(Env::default().default_filter_or("info")).init();

    setup_signal_handling();

    // Parse address used to bind exporter to.
    let addr = get_addr();

    // First self-request to make sure the server started correctly
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    {
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            trace!("waiting on client barrier");
            barrier.wait();
            let body = reqwest::blocking::get(format!("http://{addr}"))
                .unwrap()
                .text()
                .unwrap();
            info!("initial metrics look fine:\n{body}");
        });
    }

    #[cfg(feature = "sample_metrics")]
    let metric = register_gauge!("simple_the_answer", "to everything")
        .expect("can not create gauge simple_the_answer");

    #[cfg(feature = "sample_metrics")]
    let metrics = register_gauge_vec!("simple_the_answers", "to many things", &["a", "b"])
        .expect("can not create gauge simple_the_answers");

    #[cfg(feature = "sample_metrics")]
    {
        metric.set(42.0);
        metrics.with_label_values(&["A", "B"]).set(42.0);
    }

    let mut rec_metrics = StorageAccountant::new(&get_storage_dir());
    rec_metrics.update();

    // Start exporter
    let exporter = prometheus_exporter::start(addr).expect("can not start exporter");

    barrier.wait();

    loop {
        debug!("waiting for next request...");
        let guard = exporter.wait_request();
        debug!("updating metrics...");
        #[cfg(feature = "sample_metrics")]
        {
            metric.inc();
            metrics.with_label_values(&["A", "B"]).inc();
            metrics.with_label_values(&["C", "D"]).inc();
        }
        rec_metrics.update();
        debug!("updating metrics...");
        drop(guard);
    }
}
