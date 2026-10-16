//! One logging setup, so every binary prints the same shape.
//!
//! One line per record:
//!
//! ```text
//! [2026-10-16 16:43:50.123] [info ] editor::app: editor renderer: AMD Radeon Graphics
//! ```
//!
//! Default level is `info`. Third-party crates that log at `info` are quieted
//! to `warn`, so a default run is the engine's output rather than the
//! accessibility bus and the Wayland span trace. `RUST_LOG` overrides it, read
//! exactly as `env_logger` reads it.

use std::io::Write;

use log::LevelFilter;

/// Crates whose `info` output is noise for an engine, all arriving with the
/// windowing stack. A caller who wants them sets `RUST_LOG`.
const QUIET: &[&str] = &[
    "zbus",
    "zvariant",
    "tracing",
    "accesskit",
    "calloop",
    "sctk",
    "smithay_client_toolkit",
];

/// Install the process logger.
///
/// Idempotent on purpose: `try_init` fails if a logger is already set and the
/// failure is ignored, so a test harness and a `main` may both call it without
/// racing to install the same thing twice.
pub fn init() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    for name in QUIET {
        builder.filter_module(name, LevelFilter::Warn);
    }
    builder.format(|buf, record| {
        // `env_logger`'s timestamp is RFC3339; the space and the dropped `Z`
        // are the shape this log uses.
        let stamp = buf
            .timestamp_millis()
            .to_string()
            .replace('T', " ")
            .trim_end_matches('Z')
            .to_string();
        let level = record.level();
        // Colour the level and reset after it; the style is empty when the
        // output is not a terminal, so a piped log carries no escape codes.
        let style = buf.default_level_style(level);
        writeln!(
            buf,
            "[{stamp}] [{style}{level:<5}{style:#}] {}: {}",
            record.target(),
            record.args()
        )
    });
    let _ = builder.try_init();
}
