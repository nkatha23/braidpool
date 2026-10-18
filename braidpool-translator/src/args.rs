use clap::Parser;
use std::path::PathBuf;
use translator_sv2::config::TranslatorConfig;

#[derive(Parser)]
#[command(name = "braidpool-translator", about = "SV1-to-SV2 translator proxy for Braidpool")]
pub struct Args {
    /// Path to the TOML configuration file
    #[arg(short = 'c', long = "config", default_value = "braidpool-translator.toml")]
    pub config_path: PathBuf,
}

pub fn load_config() -> Result<TranslatorConfig, String> {
    let args = Args::parse();
    let contents = std::fs::read_to_string(&args.config_path)
        .map_err(|e| format!("cannot read {}: {e}", args.config_path.display()))?;
    toml::from_str(&contents).map_err(|e| format!("invalid config: {e}"))
}
