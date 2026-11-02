use clap::Parser;
use textcraft::{run_server, ServerArgs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = ServerArgs::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(args.log_level.as_str())
            }),
        )
        .init();

    run_server(args).await
}
