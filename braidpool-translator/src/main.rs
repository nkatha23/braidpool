mod args;

use translator_sv2::TranslatorSv2;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("braidpool_translator=info".parse().unwrap())
                .add_directive("translator_sv2=info".parse().unwrap()),
        )
        .init();

    let config = args::load_config().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let translator = TranslatorSv2::new(config);

    tokio::spawn({
        let t = translator.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                tracing::info!("Ctrl+C received, shutting down");
                t.shutdown().await;
            }
        }
    });

    if translator.start().await.is_err() {
        std::process::exit(1);
    }
}
