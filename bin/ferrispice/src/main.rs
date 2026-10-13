mod logger;

fn main() {
    logger::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "ferrispice");
}
