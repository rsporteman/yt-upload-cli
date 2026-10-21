//! `yt-upload` binary: wires the CLI to the OAuth flow and the API client.
//! Excluded from coverage; keep logic out of here and in `lib.rs`.

mod oauth;

use anyhow::Result;
use clap::Parser;
use yt_upload_cli::{Api, Cli, Command, auth_message, config_dir};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
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
