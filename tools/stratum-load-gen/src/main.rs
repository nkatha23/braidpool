use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio_util::sync::CancellationToken;

mod metrics;
mod miner;

use metrics::Metrics;

#[derive(Parser)]
#[command(
    name = "stratum-load-gen",
    about = "Stratum V1 load generator for Braidpool"
)]
struct Cli {
    /// Target stratum endpoint
    #[arg(long, default_value = "127.0.0.1:3333")]
    target: SocketAddr,

    /// Number of simulated miners
    #[arg(long, default_value_t = 1000)]
    miners: u64,

    /// Probability of submitting a fake share per job received (0.0–1.0)
    #[arg(long, default_value_t = 0.05)]
    submit_rate: f64,

    /// Test duration in seconds
    #[arg(long, default_value_t = 60)]
    duration: u64,

    /// Ramp-up period in seconds (miners are connected gradually over this window)
    #[arg(long, default_value_t = 10)]
    ramp_up: u64,

    /// CSV output path
    #[arg(long, default_value = "bench.csv")]
    output: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("stratum_load_gen=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    let metrics = Metrics::new();
    let shutdown = CancellationToken::new();

    let ramp_up_ms = cli.ramp_up * 1000;
    let interval_ms = if cli.miners > 0 {
        ramp_up_ms / cli.miners
    } else {
        0
    };

    tracing::info!(
        target = %cli.target,
        miners = cli.miners,
        submit_rate = cli.submit_rate,
        duration_s = cli.duration,
        ramp_up_s = cli.ramp_up,
        "starting load generator",
    );

    let mut handles = Vec::with_capacity(cli.miners as usize);

    for id in 0..cli.miners {
        if interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }
        let m = Arc::clone(&metrics);
        let s = shutdown.clone();
        let target = cli.target;
        let submit_rate = cli.submit_rate;
        handles.push(tokio::spawn(async move {
            miner::simulate_miner(id, target, submit_rate, m, s).await;
        }));
    }

    // wait for the remaining duration after ramp-up
    let elapsed_s = cli.ramp_up;
    let remaining = cli.duration.saturating_sub(elapsed_s);
    if remaining > 0 {
        tokio::time::sleep(Duration::from_secs(remaining)).await;
    }

    tracing::info!("duration elapsed, shutting down miners");
    shutdown.cancel();

    for h in handles {
        let _ = h.await;
    }

    metrics.print_summary();

    match metrics.write_csv(&cli.output) {
        Ok(()) => tracing::info!(path = cli.output, "CSV written"),
        Err(e) => tracing::error!(err = %e, "failed to write CSV"),
    }
}
