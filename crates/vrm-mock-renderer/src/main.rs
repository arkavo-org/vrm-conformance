//! vrm-mock-renderer: a deterministic mock renderer adapter that satisfies
//! the Phase 1 JSON-RPC stdio contract for testing the runner + diff +
//! S3 + site pipeline without a real renderer.

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    tracing::info!("vrm-mock-renderer starting");
    Ok(())
}
