use clap::FromArgMatches;
use mudcrab::cli::Cli;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let matches = mudcrab::cli::command().get_matches();
    let command = matches.subcommand_name().unwrap_or("mudcrab").to_string();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };
    mudcrab::ui::init(&command, cli.verbose, cli.log_file.clone());

    match mudcrab::commands::execute(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::info!("run failed: {err:#}");
            mudcrab::ui::report_error(&err);
            ExitCode::FAILURE
        }
    }
}
