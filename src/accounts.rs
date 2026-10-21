//! Saved authorizations ("accounts"). Each one is bound to a single YouTube channel and named by
//! a user-chosen alias. Several channels of one Google login and channels of different logins
//! are all just accounts, so nothing here knows about e-mail addresses.
//!
//! Layout inside the config folder: `accounts/<alias>/token.json` (yup-oauth2's token cache) and
//! `accounts/<alias>/channel.json`. An account exists when its `token.json` does.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

const TOKEN: &str = "token.json";
const CHANNEL: &str = "channel.json";
/// Alias used for the first account and for a migrated single-token setup.
pub const DEFAULT_ALIAS: &str = "default";

/// The YouTube channel an account uploads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub title: String,
    /// The `@handle`, when the channel has one.
    pub handle: Option<String>,
}

impl Channel {
    /// A channel from a `channels.list` item of the YouTube API.
    pub fn from_api(item: &Value) -> Option<Self> {
        Some(Self {
            id: item["id"].as_str()?.to_owned(),
            title: item["snippet"]["title"].as_str().unwrap_or("").to_owned(),
            handle: item["snippet"]["customUrl"].as_str().map(str::to_owned),
        })
    }

    fn from_saved(v: &Value) -> Option<Self> {
        Some(Self {
            id: v["id"].as_str()?.to_owned(),
            title: v["title"].as_str().unwrap_or("").to_owned(),
            handle: v["handle"].as_str().map(str::to_owned),
        })
    }

    fn to_saved(&self) -> Value {
        json!({ "id": self.id, "title": self.title, "handle": self.handle })
    }
}

/// Aliases are lowercase so they cannot collide on case-insensitive file systems.
pub fn validate_alias(alias: &str) -> Result<()> {
    let mut chars = alias.chars();
    let valid = alias.len() <= 32
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !valid {
        bail!(
            "invalid account name '{alias}': use 1-32 lowercase letters, digits, '-' or '_', \
             starting with a letter or digit"
        );
    }
    Ok(())
}

/// The accounts saved in a config folder.
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            root: config_dir.to_owned(),
        }
    }

    fn dir(&self, alias: &str) -> PathBuf {
        self.root.join("accounts").join(alias)
    }

    pub fn token_path(&self, alias: &str) -> PathBuf {
        self.dir(alias).join(TOKEN)
    }

    pub fn exists(&self, alias: &str) -> bool {
        self.token_path(alias).is_file()
    }

    /// Creates the account folder so the OAuth flow can save its token there.
    pub fn prepare(&self, alias: &str) -> Result<PathBuf> {
        validate_alias(alias)?;
        let dir = self.dir(alias);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(dir.join(TOKEN))
    }

    /// Saved aliases, sorted.
    pub fn aliases(&self) -> Vec<String> {
        let mut aliases: Vec<String> = std::fs::read_dir(self.root.join("accounts"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|a| validate_alias(a).is_ok() && self.exists(a))
            .collect();
        aliases.sort();
        aliases
    }

    /// The channel recorded for `alias`; `None` until known (e.g. right after a migration).
    pub fn channel(&self, alias: &str) -> Option<Channel> {
        let text = std::fs::read_to_string(self.dir(alias).join(CHANNEL)).ok()?;
        Channel::from_saved(&serde_json::from_str(&text).ok()?)
    }

    pub fn list(&self) -> Vec<(String, Option<Channel>)> {
        self.aliases()
            .into_iter()
            .map(|a| {
                let channel = self.channel(&a);
                (a, channel)
            })
            .collect()
    }

    pub fn save_channel(&self, alias: &str, channel: &Channel) -> Result<()> {
        let path = self.dir(alias).join(CHANNEL);
        std::fs::write(&path, channel.to_saved().to_string())
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Deletes everything saved for `alias`.
    pub fn remove(&self, alias: &str) -> Result<()> {
        if !self.exists(alias) {
            bail!("no account named '{alias}'; {}", self.saved_hint());
        }
        self.discard(alias)
    }

    /// Deletes the account folder, whether or not the account was complete.
    pub fn discard(&self, alias: &str) -> Result<()> {
        let dir = self.dir(alias);
        match std::fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("removing {}", dir.display()))
            }
            _ => Ok(()),
        }
    }

    fn saved_hint(&self) -> String {
        let aliases = self.aliases();
        if aliases.is_empty() {
            "no accounts are saved yet (run: yt-upload auth)".to_owned()
        } else {
            format!("saved accounts: {}", aliases.join(", "))
        }
    }
}

/// Moves the token of the single-account layout (`<config dir>/token.json`) into the account
/// `default`, so upgrading needs no new authorization. Returns whether anything moved.
pub fn migrate_legacy(store: &Store) -> Result<bool> {
    let legacy = store.root.join(TOKEN);
    if !legacy.is_file() || !store.aliases().is_empty() {
        return Ok(false);
    }
    let target = store.prepare(DEFAULT_ALIAS)?;
    std::fs::rename(&legacy, &target)
        .with_context(|| format!("moving {} to {}", legacy.display(), target.display()))?;
    Ok(true)
}

/// What the account is needed for: `auth` may create one, `upload` must use an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Auth,
    Upload,
}

/// The alias a command should use, from `--account` / `YT_UPLOAD_CLI_ACCOUNT` or the saved
/// accounts. Never guesses when several accounts are saved.
pub fn resolve(store: &Store, requested: Option<&str>, purpose: Purpose) -> Result<String> {
    let aliases = store.aliases();
    if let Some(alias) = requested {
        validate_alias(alias)?;
        if purpose == Purpose::Upload && !aliases.iter().any(|a| a == alias) {
            bail!("no account named '{alias}'; {}", store.saved_hint());
        }
        return Ok(alias.to_owned());
    }
    match aliases.as_slice() {
        [] if purpose == Purpose::Auth => Ok(DEFAULT_ALIAS.to_owned()),
        [] => bail!("no account is authorized yet; run: yt-upload auth"),
        [only] => Ok(only.clone()),
        many => bail!(
            "several accounts are saved ({}); choose one with --account <ALIAS> or \
             YT_UPLOAD_CLI_ACCOUNT",
            many.join(", ")
        ),
    }
}

/// Records the channel an authorization gave access to. A new account (`existed == false`) is
/// discarded when there is no channel or the channel is already saved under another alias.
pub fn register(
    store: &Store,
    alias: &str,
    channels: &[Channel],
    existed: bool,
) -> Result<Channel> {
    let reject = |message: String| -> Result<Channel> {
        if !existed {
            store.discard(alias)?;
        }
        bail!(message)
    };
    let Some(channel) = channels.first() else {
        return reject(format!(
            "this Google account has no YouTube channel; account '{alias}' was not saved"
        ));
    };
    if let Some((other, _)) = store
        .list()
        .into_iter()
        .find(|(a, c)| a != alias && c.as_ref().is_some_and(|c| c.id == channel.id))
    {
        return reject(format!(
            "channel '{}' is already saved as account '{other}'; account '{alias}' was not saved",
            channel.title
        ));
    }
    store.save_channel(alias, channel)?;
    Ok(channel.clone())
}

/// `accounts list` output: `alias<TAB>title<TAB>id<TAB>handle` per line, `-` when unknown.
pub fn format_list(accounts: &[(String, Option<Channel>)]) -> String {
    accounts
        .iter()
        .map(|(alias, channel)| {
            let (title, id, handle) = match channel {
                Some(c) => (c.title.as_str(), c.id.as_str(), c.handle.as_deref()),
                None => ("-", "-", None),
            };
            format!("{alias}\t{title}\t{id}\t{}\n", handle.unwrap_or("-"))
        })
        .collect()
}

/// The stderr line naming where an upload goes.
pub fn target_line(alias: &str, channel: Option<&Channel>) -> String {
    match channel {
        Some(c) => format!("Uploading to {} (account '{alias}')", c.title),
        None => format!("Uploading with account '{alias}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(id: &str) -> Channel {
        Channel {
            id: id.into(),
            title: format!("Title {id}"),
            handle: Some(format!("@{id}")),
        }
    }

    /// A store with `accounts` saved (token and, when given, channel).
    fn store_with(accounts: &[(&str, Option<Channel>)]) -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        for (alias, ch) in accounts {
            std::fs::write(store.prepare(alias).unwrap(), "{}").unwrap();
            if let Some(ch) = ch {
                store.save_channel(alias, ch).unwrap();
            }
        }
        (dir, store)
    }

    #[test]
    fn alias_rules() {
        for ok in [
            "default",
            "a",
            "work-2",
            "my_channel",
            "9lives",
            &"a".repeat(32),
        ] {
            assert!(validate_alias(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Work",
            "my channel",
            "-x",
            "_x",
            "é",
            "a/b",
            &"a".repeat(33),
        ] {
            let err = validate_alias(bad).unwrap_err().to_string();
            assert!(err.contains("invalid account name"), "{bad}");
        }
    }

    #[test]
    fn lists_sorted_with_unknown_channels() {
        let (dir, store) = store_with(&[("work", Some(channel("W"))), ("personal", None)]);
        // A folder without a token (an aborted authorization) is not an account.
        std::fs::create_dir_all(dir.path().join("accounts/aborted")).unwrap();
        std::fs::create_dir_all(dir.path().join("accounts/Bad Name")).unwrap();
        let list = store.list();
        assert_eq!(
            list,
            [
                ("personal".into(), None),
                ("work".into(), Some(channel("W")))
            ]
        );
        assert_eq!(
            format_list(&list),
            "personal\t-\t-\t-\nwork\tTitle W\tW\t@W\n"
        );
        let no_handle = Channel {
            handle: None,
            ..channel("X")
        };
        assert_eq!(
            format_list(&[("x".into(), Some(no_handle))]),
            "x\tTitle X\tX\t-\n"
        );
    }

    #[test]
    fn empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        assert!(store.list().is_empty());
        assert_eq!(format_list(&[]), "");
        assert!(store.channel("x").is_none());
    }

    #[test]
    fn removes_accounts() {
        let (_dir, store) = store_with(&[("work", Some(channel("W")))]);
        let err = store.remove("nope").unwrap_err().to_string();
        assert!(err.contains("no account named 'nope'") && err.contains("work"));
        store.remove("work").unwrap();
        assert!(!store.exists("work"));
        let err = store.remove("work").unwrap_err().to_string();
        assert!(err.contains("run: yt-upload auth"));
        store.discard("never-existed").unwrap();
    }

    #[test]
    fn migrates_the_legacy_token() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        assert!(!migrate_legacy(&store).unwrap());
        std::fs::write(dir.path().join("token.json"), "legacy").unwrap();
        assert!(migrate_legacy(&store).unwrap());
        assert!(!dir.path().join("token.json").exists());
        assert_eq!(
            std::fs::read_to_string(store.token_path("default")).unwrap(),
            "legacy"
        );
        assert_eq!(store.list(), [("default".into(), None)]);
        // Once accounts exist, a stray legacy token is left alone.
        std::fs::write(dir.path().join("token.json"), "other").unwrap();
        assert!(!migrate_legacy(&store).unwrap());
    }

    #[test]
    fn resolves_accounts() {
        use Purpose::{Auth, Upload};
        let (_d0, none) = store_with(&[]);
        assert_eq!(resolve(&none, None, Auth).unwrap(), "default");
        assert!(
            resolve(&none, None, Upload)
                .unwrap_err()
                .to_string()
                .contains("run: yt-upload auth")
        );
        assert_eq!(resolve(&none, Some("new"), Auth).unwrap(), "new");
        assert!(resolve(&none, Some("Bad"), Auth).is_err());

        let (_d1, one) = store_with(&[("work", None)]);
        for purpose in [Auth, Upload] {
            assert_eq!(resolve(&one, None, purpose).unwrap(), "work");
        }

        let (_d2, two) = store_with(&[("work", None), ("personal", None)]);
        for purpose in [Auth, Upload] {
            let err = resolve(&two, None, purpose).unwrap_err().to_string();
            assert!(
                err.contains("personal, work") && err.contains("--account"),
                "{err}"
            );
        }
        assert_eq!(resolve(&two, Some("work"), Upload).unwrap(), "work");
        let err = resolve(&two, Some("nope"), Upload).unwrap_err().to_string();
        assert!(err.contains("'nope'") && err.contains("personal, work"));
        assert_eq!(resolve(&two, Some("nope"), Auth).unwrap(), "nope");
    }

    #[test]
    fn registers_channels() {
        let (_dir, store) = store_with(&[("work", Some(channel("W"))), ("new", None)]);

        let err = register(&store, "new", &[], false).unwrap_err().to_string();
        assert!(err.contains("no YouTube channel"));
        assert!(!store.exists("new"));

        store.prepare("new").unwrap();
        std::fs::write(store.token_path("new"), "{}").unwrap();
        let err = register(&store, "new", &[channel("W")], false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("already saved as account 'work'"), "{err}");
        assert!(!store.exists("new"));

        // Refreshing an existing account never deletes it.
        let err = register(&store, "work", &[], true).unwrap_err().to_string();
        assert!(err.contains("no YouTube channel"));
        assert!(store.exists("work"));

        assert_eq!(
            register(&store, "work", &[channel("W")], true).unwrap(),
            channel("W")
        );
        store.prepare("other").unwrap();
        std::fs::write(store.token_path("other"), "{}").unwrap();
        assert_eq!(
            register(&store, "other", &[channel("O"), channel("P")], false).unwrap(),
            channel("O")
        );
        assert_eq!(store.channel("other"), Some(channel("O")));
    }

    #[test]
    fn parses_api_channels_and_target_lines() {
        let item = json!({ "id": "UC1", "snippet": { "title": "My", "customUrl": "@my" } });
        assert_eq!(
            Channel::from_api(&item),
            Some(Channel {
                id: "UC1".into(),
                title: "My".into(),
                handle: Some("@my".into())
            })
        );
        assert_eq!(Channel::from_api(&json!({ "snippet": {} })), None);
        assert_eq!(
            target_line("work", Some(&channel("W"))),
            "Uploading to Title W (account 'work')"
        );
        assert_eq!(
            target_line("default", None),
            "Uploading with account 'default'"
        );
    }
}
