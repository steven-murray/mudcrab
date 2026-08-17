use crate::cli::IdentifyArgs;
use crate::config::identify::{identify, write_meta_sidecar};

pub async fn run(args: IdentifyArgs) -> anyhow::Result<()> {
    let api_key = std::env::var("NEXUS_API_KEY").map_err(|_| {
        anyhow::anyhow!(
            "NEXUS_API_KEY must be set: identifying an archive is a Nexus API call, \
             the same one Mod Organizer 2 makes when it recognises a file you \
             downloaded by hand."
        )
    })?;

    let client = reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .map_err(|err| anyhow::anyhow!("failed to build an http client: {err}"))?;

    let mut failures = 0usize;
    for archive in &args.archives {
        if !archive.is_file() {
            eprintln!("{}: not a file", archive.display());
            failures += 1;
            continue;
        }

        match identify(&client, archive, &args.game, &api_key, args.api_base.as_deref()).await {
            Ok(found) => {
                if let Some(name) = &found.mod_name {
                    println!("# {name}");
                }
                print!("{}", found.toml_snippet());

                if args.write_meta {
                    let sidecar = write_meta_sidecar(archive, &found)?;
                    eprintln!("wrote {}", sidecar.display());
                }
                println!();
            }
            Err(err) => {
                // One unrecognised archive should not cost the rest of the batch.
                eprintln!("{}: {err}", archive.display());
                failures += 1;
            }
        }
    }

    if failures > 0 {
        anyhow::bail!("{failures} archive(s) could not be identified");
    }
    Ok(())
}
