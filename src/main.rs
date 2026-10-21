//! `yt-upload` binary: wires the CLI to the OAuth flow and the API client.
//! Excluded from coverage; keep logic out of here and in `lib.rs`.

mod oauth;

use anyhow::Result;
use yt_upload_cli::{Api, Command, auth_message, config_dir, env, parse_cli};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_cli(std::env::args_os(), env::os_lookup).unwrap_or_else(|e| e.exit());
    let dir = config_dir(cli.config_dir)?;
    match cli.command {
        Command::ConfigDir => println!("{}", dir.display()),
        Command::Auth => {
            let api = Api::new(oauth::access_token(&dir).await?)?;
            println!("{}", auth_message(&api.channel_names().await?));
        }
        Command::Upload(args) => {
            let api = Api::new(oauth::access_token(&dir).await?)?;
            api.upload(&args).await?;
        }
    }
    Ok(())
}
