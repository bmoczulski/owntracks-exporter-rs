# What?

This is Prometheus exporter for OwnTracks Recorder.

# Why?

Because OwnTracks Recorder doesn't expose it and I want to monitor my instance in Prometheus/Grafana - e.g. alert if any of the clients stops reporting, maybe due to misconfiguration.

\- Hey, but there is one already! https://github.com/linusg/prometheus-owntracks-exporter  
\- Yes, but I'd like to have per-client separation with Prometheus labels, which the above is missing.  

\- Why not simply improve the above and raise a pull-request?  
\- See the next answer.

\- Why Rust? What's wrong with Python, Node.js, or even PHP?  
\- Because it was a great excuse to pick some Rust at last :) One does not simply pass on such an opportunity!

# How to run with Docker

```bash
docker run --rm -p 9192:9192 -v $YOUR_OWNTRACKS_RECORDER_STORAGE:/otr-storage:ro moczulski/owntracks-exporter
```

Environment variables:

| variable                       | default value  | meaning                                                                   |
|--------------------------------|----------------|---------------------------------------------------------------------------|
| OWNTRACKS_EXPORTER_BIND_HOST   | `0.0.0.0`      | server bind address                                                       |
| OWNTRACKS_EXPORTER_BIND_PORT   | `9192`         | server bind port                                                          |
| OWNTRACKS_EXPORTER_STORAGE_DIR | `/otr-storage` | where you keep your OwnTracks Recorder data                               |
| RUST_LOG                       | `info`         | `env_logger` verbosity, can be: `error`, `warn`, `info`, `debug`, `trace` |

# How to run with cargo?

If you insist, the usual Rust way:

```bash
cargo run
```

Above variables still work of course, e.g.:

```bash
OWNTRACKS_EXPORTER_BIND_PORT=9999 cargo run
```

# How to check it's running?

Nothing worring in startup logs? Just make an HTTP request to the endpoint:

```
curl http://localhost:9192/metrics
```

# How to add it to Prometheus?

Add new scrape config in `/etc/prometheus/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: "owntracks-recorder"
    static_configs:
    - targets: ["localhost:9192"]
```
