use std::io;

use tracing_subscriber::{EnvFilter, filter::LevelFilter};

pub fn init() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(io::stderr)
        .try_init();
}
