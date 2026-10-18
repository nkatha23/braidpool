use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct CsvRow {
    ts_epoch_ms: u64,
    miner_id: u64,
    event: &'static str,
    value_us: u64,
}

pub struct Metrics {
    rows: Mutex<Vec<CsvRow>>,
    pub connect_failures: AtomicU64,
    pub miners_connected: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rows: Mutex::new(Vec::new()),
            connect_failures: AtomicU64::new(0),
            miners_connected: AtomicU64::new(0),
        })
    }

    pub fn record(&self, miner_id: u64, event: &'static str, value_us: u64) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.rows.lock().unwrap().push(CsvRow {
            ts_epoch_ms: ts,
            miner_id,
            event,
            value_us,
        });
    }

    pub fn write_csv(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let rows = self.rows.lock().unwrap();
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "ts_epoch_ms,miner_id,event,value_us")?;
        for r in rows.iter() {
            writeln!(
                f,
                "{},{},{},{}",
                r.ts_epoch_ms, r.miner_id, r.event, r.value_us
            )?;
        }
        Ok(())
    }

    pub fn print_summary(&self) {
        let rows = self.rows.lock().unwrap();
        for event in [
            "connect_latency",
            "subscribe_latency",
            "job_interval",
            "submit_rtt",
        ] {
            let mut values: Vec<u64> = rows
                .iter()
                .filter(|r| r.event == event)
                .map(|r| r.value_us)
                .collect();
            if values.is_empty() {
                continue;
            }
            values.sort_unstable();
            let n = values.len();
            println!(
                "{event}: count={n} p50={}μs p95={}μs p99={}μs max={}μs",
                values[n * 50 / 100],
                values[(n * 95 / 100).min(n - 1)],
                values[(n * 99 / 100).min(n - 1)],
                values[n - 1],
            );
        }
        println!(
            "miners_connected={} connect_failures={}",
            self.miners_connected.load(Ordering::Relaxed),
            self.connect_failures.load(Ordering::Relaxed),
        );
    }
}
