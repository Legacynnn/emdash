//! EMD-18 load test for the PTY streaming primitive.
//!
//! Spawns N concurrent PTYs piping `yes` (or another high-rate
//! producer) for D seconds, with the same `Coalescer` parameters as
//! the production `pty_spawn` path. Measures:
//!
//! - bytes received per stream (after coalescing) and aggregate
//!   throughput (MiB/s)
//! - max consecutive "in-callback" time per stream (proxy for the
//!   renderer-side frame-time risk: a long synchronous receive
//!   blocks the webview thread that would be reading the channel)
//! - dropped byte count (always 0 on the host side — coalescing is
//!   loss-less; renderer drops would show up as a `Channel::send`
//!   failure that we don't surface here, by design — see ADR-0003)
//!
//! Renderer-side measurements (true frame-time) can't be made
//! without a webview attached. The harness mirrors what the
//! production `commands::pty::pty_spawn` does as closely as it can
//! from outside the Tauri runtime.
//!
//! Usage:
//!   cargo run --bin pty-loadtest -- [--streams N] [--duration-secs S] [--cmd CMD]
//!
//! Defaults: N=10, S=10, CMD="yes". Use `--duration-secs 60` for the
//! full EMD-18 acceptance run; the 10 s default is the CI-friendly
//! quick check.

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use emdash_dev::pty::registry::Registry;
use emdash_dev::pty::types::{PtySize, SpawnOptions};

#[derive(Debug)]
struct Config {
    streams: usize,
    duration: Duration,
    command: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            streams: 10,
            duration: Duration::from_secs(10),
            command: "yes".to_string(),
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--streams" => {
                let v = args.next().ok_or("--streams takes a value")?;
                cfg.streams = v.parse().map_err(|e| format!("--streams: {e}"))?;
            }
            "--duration-secs" => {
                let v = args.next().ok_or("--duration-secs takes a value")?;
                let s: u64 = v.parse().map_err(|e| format!("--duration-secs: {e}"))?;
                cfg.duration = Duration::from_secs(s);
            }
            "--cmd" => {
                cfg.command = args.next().ok_or("--cmd takes a value")?;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(cfg)
}

fn print_help() {
    println!(
        "pty-loadtest — EMD-18 streaming-primitive validator\n\
         \n\
         USAGE:\n\
             pty-loadtest [--streams N] [--duration-secs S] [--cmd CMD]\n\
         \n\
         DEFAULTS:\n\
             --streams 10 --duration-secs 10 --cmd yes\n\
         \n\
         The 60 s × 10-stream run is the EMD-18 acceptance scenario;\n\
         shorter runs are useful for smoke-checking CI."
    );
}

struct StreamStats {
    bytes: AtomicU64,
    /// Max time the receive callback spent before returning, in
    /// microseconds. Long callbacks are the renderer-side proxy:
    /// if the host's per-callback work is ~free, the renderer will
    /// only stall on its own work (JSON decode, terminal repaint).
    max_callback_us: AtomicU64,
}

impl StreamStats {
    fn new() -> Self {
        Self {
            bytes: AtomicU64::new(0),
            max_callback_us: AtomicU64::new(0),
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::FAILURE;
        }
    };

    println!(
        "pty-loadtest: {streams} streams × {duration} s, cmd={cmd:?}",
        streams = cfg.streams,
        duration = cfg.duration.as_secs(),
        cmd = cfg.command,
    );

    let registry = Arc::new(Registry::new());
    let mut stats: HashMap<u32, Arc<StreamStats>> = HashMap::new();

    let opts = SpawnOptions {
        command: cfg.command.clone(),
        args: Vec::new(),
        cwd: None,
        env: HashMap::new(),
        size: PtySize { rows: 24, cols: 80 },
    };

    for _ in 0..cfg.streams {
        let s = Arc::new(StreamStats::new());
        let cb_stats = s.clone();
        let id = match registry.spawn(opts.clone(), move |bytes| {
            let t0 = Instant::now();
            cb_stats
                .bytes
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
            let elapsed_us = t0.elapsed().as_micros() as u64;
            cb_stats
                .max_callback_us
                .fetch_max(elapsed_us, Ordering::Relaxed);
        }) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("spawn failed: {e:?}");
                registry.drain();
                return ExitCode::FAILURE;
            }
        };
        stats.insert(id.0, s);
    }

    println!("running for {} s...", cfg.duration.as_secs());
    let t_start = Instant::now();
    tokio::time::sleep(cfg.duration).await;
    let wall = t_start.elapsed().as_secs_f64();

    println!("draining registry...");
    registry.drain();

    println!("\n=== EMD-18 results ===");
    println!("wall_time_s              = {wall:.3}");
    println!("streams                  = {}", cfg.streams);
    println!("command                  = {:?}", cfg.command);
    println!();

    let mut total_bytes: u64 = 0;
    let mut max_callback_us_any: u64 = 0;
    for (id, s) in &stats {
        let b = s.bytes.load(Ordering::Relaxed);
        let cb = s.max_callback_us.load(Ordering::Relaxed);
        total_bytes += b;
        max_callback_us_any = max_callback_us_any.max(cb);
        println!(
            "  stream id={id:>3}  bytes={b:>11}  MiB/s={:>7.2}  max_cb_us={cb}",
            (b as f64) / wall / (1024.0 * 1024.0)
        );
    }

    let throughput_mibs = (total_bytes as f64) / wall / (1024.0 * 1024.0);
    println!();
    println!("total_bytes              = {total_bytes}");
    println!("aggregate_MiB_per_s      = {throughput_mibs:.2}");
    println!("max_per_stream_callback_us_observed = {max_callback_us_any}");
    println!("dropped_bytes            = 0  (coalescer is loss-less on the host side)");
    println!();
    println!(
        "callback_us threshold: <50_000 (~50 ms = one dropped 20fps frame). \
         observed max: {max_callback_us_any} us {marker}",
        marker = if max_callback_us_any < 50_000 {
            "OK"
        } else {
            "WARN"
        }
    );

    if max_callback_us_any >= 50_000 {
        // Non-blocking by design — the harness reports the warning but
        // doesn't fail the run. CI consumes the warning if present.
        eprintln!("warning: max callback latency exceeded the 50ms threshold");
    }
    ExitCode::SUCCESS
}
