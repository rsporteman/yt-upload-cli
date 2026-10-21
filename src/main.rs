//! `yt-upload` binary: wires the CLI to the OAuth flow and the API client.
//! Excluded from coverage; keep logic out of here and in `lib.rs`.

mod oauth;

use anyhow::Result;
use yt_upload_cli::accounts::{self, Purpose, Store};
use yt_upload_cli::{AccountsCommand, Api, Command, auth_message, config_dir, env, parse_cli};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_cli(std::env::args_os(), env::os_lookup).unwrap_or_else(|e| e.exit());
    let dir = config_dir(cli.config_dir)?;
    let store = Store::new(&dir);
    if !matches!(cli.command, Command::ConfigDir) && accounts::migrate_legacy(&store)? {
        eprintln!(
            "Moved the saved token to account '{}'.",
            accounts::DEFAULT_ALIAS
        );
    }
    match cli.command {
        Command::ConfigDir => println!("{}", dir.display()),
        Command::Auth(args) => {
            let alias = accounts::resolve(&store, args.account.as_deref(), Purpose::Auth)?;
            let existed = store.exists(&alias);
            let token = oauth::access_token(&dir, &store.prepare(&alias)?).await;
            let token = token.inspect_err(|_| {
                if !existed {
                    let _ = store.discard(&alias);
                }
            })?;
            let channels = Api::new(token)?.channels().await?;
            let channel = accounts::register(&store, &alias, &channels, existed)?;
            println!("{}", auth_message(&channel));
            eprintln!("Saved as account '{alias}'.");
        }
        Command::Upload(args) => {
            let alias = accounts::resolve(&store, args.account.as_deref(), Purpose::Upload)?;
            eprintln!(
                "{}",
                accounts::target_line(&alias, store.channel(&alias).as_ref())
            );
            let api = Api::new(oauth::access_token(&dir, &store.token_path(&alias)).await?)?;
            api.upload(&args).await?;
        }
        Command::Accounts(AccountsCommand::List) => {
            let list = store.list();
            if list.is_empty() {
                eprintln!("No accounts saved yet; run: yt-upload auth --account <ALIAS>");
            }
            print!("{}", accounts::format_list(&list));
        }
        Command::Accounts(AccountsCommand::Remove { alias }) => {
            store.remove(&alias)?;
            eprintln!(
                "Removed account '{alias}' from this computer. To also revoke yt-upload's access, \
                 visit https://myaccount.google.com/permissions"
            );
        }
    }
    Ok(())
}
