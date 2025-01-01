// Will create an exporter with a single metric that does not change

use env_logger::{
    Builder,
    Env,
};
use log::{error, debug, info, trace};
use prometheus_exporter::prometheus::{
    CounterVec,
    register_counter_vec, register_gauge, register_gauge_vec
};
use std::{collections::HashMap, fs, net::SocketAddr, path::Path};

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

fn get_storage_dir() -> String {
    std::env::var("OWNTRACKS_EXPORTER_STORAGE_DIR")
        .unwrap_or("/otr-storage".to_string())
}

struct StorageAccountant {
    root: String,
    m_points_total: CounterVec,
}

#[derive(Eq, Hash, PartialEq)]
struct StorageDevice {
    user_name: String,
    device_name: String
}

impl StorageAccountant {
    fn new(root: &str) -> Self {
        Self {
            root: root.to_owned(),
            m_points_total: register_counter_vec!("owntracks_recorder_points_total", "Total number of points recorded so far", &["user", "device"]).unwrap(),
        }
    }

    fn get_all_subdirs(dir: &Path) -> Vec<String> {
        let mut subdirs : Vec<String> = Vec::new();
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_dir() {
                                match path.file_name() {
                                    Some(basename) => {
                                        trace!("found dir: {}/{}", dir.as_os_str().to_str().unwrap_or("?"), basename.to_str().unwrap_or("?"));
                                        match basename.to_str() {
                                            Some(basename) => subdirs.push(basename.to_owned()),
                                            None => ()
                                        }
                                    }
                                    None => ()
                                }
                            }
                        }
                        Err(e) => error!("Error dir entry: {}", e)
                    }
                }
            }
            Err(e) => error!("Error read dir: {}", e),
        }
        subdirs
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
                devices.push(StorageDevice{
                    user_name: user_name.clone(),
                    device_name: device_name
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

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>>{
        for device in self.get_devices() {
            let labels = Self::to_labels_map(&device);
            let m = self.m_points_total.with(&labels);
            m.inc();
        }
        Ok(())
    }
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

    let mut rec_metrics = StorageAccountant::new(&get_storage_dir());
    let _ = rec_metrics.update();

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
        let _ = rec_metrics.update();
        debug!("updating metrics...");
        drop(guard);
    }
}