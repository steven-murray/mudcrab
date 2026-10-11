use crate::cli::NexusFilesArgs;
use crate::config::identify::nexus_files;

pub async fn run(args: NexusFilesArgs) -> anyhow::Result<()> {
    let api_key = std::env::var("NEXUS_API_KEY").map_err(|_| {
        anyhow::anyhow!("NEXUS_API_KEY must be set: listing a mod's files is a Nexus API call")
    })?;
    let client = reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .map_err(|err| anyhow::anyhow!("failed to build an http client: {err}"))?;

    let mut files = nexus_files(
        &client,
        args.api_base.as_deref(),
        &api_key,
        &args.game,
        args.mod_id,
    )
    .await?;
    let total = files.len();
    // Old and archived files are what a modlist should not be pinning by
    // accident; hidden unless asked for, and counted so they are not a secret.
    if !args.all {
        files.retain(|file| {
            !matches!(
                file.category_name.as_deref(),
                Some("OLD_VERSION" | "ARCHIVED" | "DELETED")
            )
        });
    }
    // Newest first within a category reads best, and file ids increase with
    // upload time.
    files.sort_by(|a, b| {
        category_rank(a.category_name.as_deref())
            .cmp(&category_rank(b.category_name.as_deref()))
            .then(b.file_id.cmp(&a.file_id))
    });

    for file in &files {
        println!(
            "{:<13} {}  {}{}",
            file.category_name.as_deref().unwrap_or("?"),
            file.name.as_deref().unwrap_or("(untitled)"),
            file.version
                .as_deref()
                .map(|v| format!("v{v}"))
                .unwrap_or_default(),
            file.size_kb
                .map(|kb| format!("  {:.1} MB", kb as f64 / 1024.0))
                .unwrap_or_default(),
        );
        println!(
            "              path = \"nexus:{}/{}/{}\"",
            args.game, args.mod_id, file.file_id
        );
        if let Some(name) = &file.file_name {
            println!(
                "              file_name = {}",
                toml::Value::String(name.clone())
            );
        }
    }
    if files.len() < total {
        println!(
            "({} old or archived file(s) not shown; --all lists them)",
            total - files.len()
        );
    }
    Ok(())
}

fn category_rank(category: Option<&str>) -> u8 {
    match category {
        Some("MAIN") => 0,
        Some("UPDATE") => 1,
        Some("OPTIONAL") => 2,
        Some("MISCELLANEOUS") => 3,
        _ => 4,
    }
}
