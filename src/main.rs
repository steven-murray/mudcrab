use clap::Parser;
use mudcrab::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mudcrab::init_tracing();

    let cli = Cli::parse();
    mudcrab::commands::execute(cli).await
}
