//! Library behind the `yt-upload` binary: the CLI definition and the YouTube Data API v3 calls.
//! The interactive OAuth flow lives in the binary (`src/oauth.rs`); everything here is testable
//! against a mock server.

pub mod accounts;
pub mod env;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use accounts::Channel;
use anyhow::{Context, Result, bail};
use clap::builder::BoolishValueParser;
use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{Arg, ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use env::Persisted;
use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;

pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/youtube.upload",
    "https://www.googleapis.com/auth/youtube",
];
const API: &str = "https://www.googleapis.com/youtube/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/youtube/v3";
/// Resumable upload chunks must be a multiple of 256 KiB.
const CHUNK_SIZE: usize = 32 * 256 * 1024;

#[derive(Parser, Debug)]
#[command(
    name = "yt-upload",
    version,
    about = "Upload videos to YouTube from the command line"
)]
pub struct Cli {
    /// Folder holding client_secret.json and the saved accounts
    /// [default: <config dir>/yt-upload-cli]
    #[arg(long, env = "YT_UPLOAD_CLI_HOME", global = true)]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Authorize a YouTube channel and save it as an account
    Auth(AuthArgs),
    /// Upload a video
    Upload(Box<UploadArgs>),
    /// List or remove saved accounts
    #[command(subcommand)]
    Accounts(AccountsCommand),
    /// Print the config folder path
    ConfigDir,
}

#[derive(clap::Args, Debug)]
pub struct AuthArgs {
    /// Name to save this account under (lowercase letters, digits, '-' or '_')
    /// [default: the only saved account, or "default" when there is none]
    #[arg(long, env = "YT_UPLOAD_CLI_ACCOUNT")]
    pub account: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum AccountsCommand {
    /// Print saved accounts: alias, channel title, channel id and handle, tab-separated
    List,
    /// Delete a saved account from this computer
    Remove {
        /// Account to remove
        alias: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct UploadArgs {
    /// Video file to upload
    pub video: PathBuf,
    /// Account to upload with; required when more than one account is saved
    #[arg(long, env = "YT_UPLOAD_CLI_ACCOUNT")]
    pub account: Option<String>,
    /// Video title
    #[arg(short, long)]
    pub title: String,
    /// Video description
    #[arg(short, long, default_value = "")]
    pub description: String,
    /// Comma-separated tags
    #[arg(long, env = "YT_UPLOAD_CLI_TAGS", default_value = "")]
    pub tags: String,
    /// Who can watch the video
    #[arg(short, long, env = "YT_UPLOAD_CLI_PRIVACY", value_enum, default_value_t = Privacy::Private)]
    pub privacy: Privacy,
    /// Category id (22 = People & Blogs)
    #[arg(long, env = "YT_UPLOAD_CLI_CATEGORY", default_value = "22")]
    pub category: String,
    /// Thumbnail image (JPG or PNG, up to 2 MB)
    #[arg(long)]
    pub thumbnail: Option<PathBuf>,
    /// Playlist id to add the video to
    #[arg(long, env = "YT_UPLOAD_CLI_PLAYLIST")]
    pub playlist: Option<String>,
    /// Schedule publishing (RFC 3339, e.g. 2026-10-25T15:00:00Z); forces private until then
    #[arg(long)]
    pub publish_at: Option<String>,
    /// Mark the video as made for kids
    #[arg(long, env = "YT_UPLOAD_CLI_MADE_FOR_KIDS", value_parser = BoolishValueParser::new())]
    pub made_for_kids: bool,
}

#[derive(Clone, Copy, ValueEnum, Debug, PartialEq, Eq)]
pub enum Privacy {
    Private,
    Unlisted,
    Public,
}

impl Privacy {
    pub fn as_str(self) -> &'static str {
        match self {
            Privacy::Private => "private",
            Privacy::Unlisted => "unlisted",
            Privacy::Public => "public",
        }
    }
}

const HOME_VAR: &str = "YT_UPLOAD_CLI_HOME";
const ACCOUNT_VAR: &str = "YT_UPLOAD_CLI_ACCOUNT";
const PRIVACY_VAR: &str = "YT_UPLOAD_CLI_PRIVACY";
const CATEGORY_VAR: &str = "YT_UPLOAD_CLI_CATEGORY";
const TAGS_VAR: &str = "YT_UPLOAD_CLI_TAGS";
const PLAYLIST_VAR: &str = "YT_UPLOAD_CLI_PLAYLIST";
const MADE_FOR_KIDS_VAR: &str = "YT_UPLOAD_CLI_MADE_FOR_KIDS";

/// Parses the command line. Precedence per option: flag, process environment (clap), then
/// `lookup` (the OS-persisted environment), then the built-in default.
pub fn parse_cli<I, T>(
    args: I,
    lookup: impl Fn(&str) -> Option<Persisted>,
) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = command().try_get_matches_from(args)?;
    let mut cli = Cli::from_arg_matches(&matches)?;
    let fallback = |m: &ArgMatches, id: &str, var: &str| {
        if matches!(m.value_source(id), None | Some(ValueSource::DefaultValue)) {
            lookup(var).filter(|p| !p.value.is_empty())
        } else {
            None
        }
    };

    if let Some(p) = fallback(&matches, "config_dir", HOME_VAR) {
        cli.config_dir = Some(p.value.into());
    }
    if let (Command::Auth(args), Some(("auth", m))) = (&mut cli.command, matches.subcommand())
        && let Some(p) = fallback(m, "account", ACCOUNT_VAR)
    {
        args.account = Some(p.value);
    }
    if let (Command::Upload(args), Some(("upload", m))) = (&mut cli.command, matches.subcommand()) {
        if let Some(p) = fallback(m, "account", ACCOUNT_VAR) {
            args.account = Some(p.value);
        }
        if let Some(p) = fallback(m, "privacy", PRIVACY_VAR) {
            args.privacy = Privacy::from_str(&p.value, true)
                .map_err(|_| invalid_persisted(PRIVACY_VAR, &p, "private, unlisted, public"))?;
        }
        if let Some(p) = fallback(m, "category", CATEGORY_VAR) {
            args.category = p.value;
        }
        if let Some(p) = fallback(m, "tags", TAGS_VAR) {
            args.tags = p.value;
        }
        if let Some(p) = fallback(m, "playlist", PLAYLIST_VAR) {
            args.playlist = Some(p.value);
        }
        if let Some(p) = fallback(m, "made_for_kids", MADE_FOR_KIDS_VAR) {
            args.made_for_kids = parse_bool(&p.value).ok_or_else(|| {
                invalid_persisted(MADE_FOR_KIDS_VAR, &p, "true, false, 1, 0, yes, no, on, off")
            })?;
        }
    }
    Ok(cli)
}

/// The clap command, ignoring variables that are set but empty: clap would pass them on as an
/// empty value (an error for most options), while users expect "empty" to mean "unset".
fn command() -> clap::Command {
    fn unset_if_empty(arg: Arg) -> Arg {
        match arg.get_env() {
            Some(var) if std::env::var_os(var).is_some_and(|v| v.is_empty()) => {
                arg.env(None::<&'static str>)
            }
            _ => arg,
        }
    }
    Cli::command()
        .mut_args(unset_if_empty)
        .mut_subcommand("auth", |s| s.mut_args(unset_if_empty))
        .mut_subcommand("upload", |s| s.mut_args(unset_if_empty))
}

fn invalid_persisted(var: &str, p: &Persisted, allowed: &str) -> clap::Error {
    clap::Error::raw(
        ErrorKind::InvalidValue,
        format!(
            "invalid value '{}' for {var} in the OS environment ({}); possible values: {allowed}\n",
            p.value, p.source
        ),
    )
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// The folder given by the user, or `<user config dir>/yt-upload-cli`.
pub fn config_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    match explicit {
        Some(dir) => Ok(dir),
        None => dirs::config_dir()
            .map(|d| d.join("yt-upload-cli"))
            .context("could not find the user config folder; use --config-dir"),
    }
}

/// Path of the OAuth client file, or an error explaining how to create it.
pub fn client_secret_path(dir: &Path) -> Result<PathBuf> {
    let path = dir.join("client_secret.json");
    if !path.exists() {
        bail!(
            "missing {}\nCreate a Desktop OAuth client in Google Cloud and save its JSON there.",
            path.display()
        );
    }
    Ok(path)
}

/// Message shown by `yt-upload auth` for the authorized channel.
pub fn auth_message(channel: &Channel) -> String {
    format!("Authorized. Channel: {}", channel.title)
}

/// Makes Google's consent page always show its account chooser, so another Google login can be
/// picked for a new account; `consent` also gets a refresh token for a login that authorized
/// the app before.
pub fn with_account_chooser(url: &str) -> String {
    if url.contains("prompt=") {
        return url.to_owned();
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}prompt=select_account%20consent")
}

/// The `videos.insert` request body for these arguments.
pub fn video_body(args: &UploadArgs) -> Value {
    let mut status = json!({
        "privacyStatus": args.privacy.as_str(),
        "selfDeclaredMadeForKids": args.made_for_kids,
    });
    if let Some(at) = &args.publish_at {
        // YouTube only schedules private videos; they turn public at `publishAt`.
        status["privacyStatus"] = json!("private");
        status["publishAt"] = json!(at);
    }
    let tags: Vec<&str> = args
        .tags
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    json!({
        "snippet": {
            "title": args.title,
            "description": args.description,
            "tags": tags,
            "categoryId": args.category,
        },
        "status": status,
    })
}

/// Largest thumbnail YouTube accepts.
const MAX_THUMBNAIL_BYTES: u64 = 2 * 1024 * 1024;

/// Checks the thumbnail before anything is uploaded, so a bad file does not cost an upload,
/// and returns its MIME type.
pub fn validate_thumbnail(path: &Path) -> Result<&'static str> {
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => bail!("{}: thumbnail must be a JPG or PNG file", path.display()),
    };
    let len = std::fs::File::open(path)
        .and_then(|f| f.metadata())
        .with_context(|| format!("opening thumbnail {}", path.display()))?
        .len();
    if len > MAX_THUMBNAIL_BYTES {
        bail!("{}: thumbnail exceeds 2 MB", path.display());
    }
    Ok(mime)
}

/// An authenticated client for the YouTube Data API.
pub struct Api {
    http: Client,
    token: String,
    api_base: String,
    upload_base: String,
    chunk_size: usize,
}

impl Api {
    /// Client for Google's production endpoints.
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_endpoints(token, API, UPLOAD_API, CHUNK_SIZE)
    }

    /// Client for custom endpoints, used by tests.
    pub fn with_endpoints(
        token: impl Into<String>,
        api_base: &str,
        upload_base: &str,
        chunk_size: usize,
    ) -> Result<Self> {
        // YouTube answers 308 "Resume Incomplete" between chunks; it must not be followed.
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            token: token.into(),
            api_base: api_base.to_owned(),
            upload_base: upload_base.to_owned(),
            chunk_size,
        })
    }

    /// The channels the token can access (the one picked at consent time).
    pub async fn channels(&self) -> Result<Vec<Channel>> {
        let resp = self
            .http
            .get(format!("{}/channels?part=snippet&mine=true", self.api_base))
            .bearer_auth(&self.token)
            .send()
            .await?;
        let channels: Value = check(resp).await?.json().await?;
        Ok(channels["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Channel::from_api)
            .collect())
    }

    /// Uploads the video, then sets the thumbnail and playlist if requested.
    /// Returns the new video id. Progress goes to stderr.
    pub async fn upload(&self, args: &UploadArgs) -> Result<String> {
        let thumbnail = match &args.thumbnail {
            Some(path) => Some((path, validate_thumbnail(path)?)),
            None => None,
        };
        let mut file = tokio::fs::File::open(&args.video)
            .await
            .with_context(|| format!("opening {}", args.video.display()))?;
        let total = file.metadata().await?.len();
        if total == 0 {
            bail!("{} is empty", args.video.display());
        }

        // Start a resumable session; Google answers with the upload URL in Location.
        let resp = self
            .http
            .post(format!(
                "{}/videos?uploadType=resumable&part=snippet,status",
                self.upload_base
            ))
            .bearer_auth(&self.token)
            .header("X-Upload-Content-Length", total)
            .header("X-Upload-Content-Type", "video/*")
            .json(&video_body(args))
            .send()
            .await?;
        let resp = check(resp).await?;
        let session = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .context("Google did not return an upload URL")?
            .to_owned();

        let mut sent: u64 = 0;
        let mut buf = vec![0u8; self.chunk_size];
        let video: Value = loop {
            let n = read_full(&mut file, &mut buf).await?;
            let end = sent + n as u64;
            let resp = self
                .http
                .put(&session)
                .bearer_auth(&self.token)
                .header(header::CONTENT_RANGE, content_range(sent, n, total))
                .body(buf[..n].to_vec())
                .send()
                .await?;
            if resp.status() == StatusCode::PERMANENT_REDIRECT {
                sent = end;
                eprintln!("Uploading... {}%", sent * 100 / total);
                continue;
            }
            break check(resp).await?.json().await?;
        };
        let id = video["id"]
            .as_str()
            .context("upload finished without a video id")?
            .to_owned();
        println!("Uploaded: https://youtu.be/{id}");

        // The video is already up: a thumbnail failure must not skip the playlist, and a
        // channel that may not set custom thumbnails is not a failure at all.
        let mut thumbnail_error = None;
        if let Some((path, mime)) = thumbnail {
            match self.set_thumbnail(&id, path, mime).await {
                Ok(()) => println!("Thumbnail set."),
                Err(e) if is_thumbnail_not_allowed(&e) => eprintln!(
                    "Warning: thumbnail not set: {e}\n\
                     Custom thumbnails need a verified channel: https://www.youtube.com/verify"
                ),
                Err(e) => thumbnail_error = Some(e.context("setting the thumbnail")),
            }
        }
        if let Some(playlist) = &args.playlist {
            match self.add_to_playlist(&id, playlist).await {
                Ok(()) => println!("Added to playlist."),
                Err(e) => {
                    return Err(match thumbnail_error {
                        Some(t) => t.context(format!("adding to the playlist also failed: {e}")),
                        None => e,
                    });
                }
            }
        }
        match thumbnail_error {
            Some(e) => Err(e),
            None => Ok(id),
        }
    }

    async fn set_thumbnail(&self, video_id: &str, path: &Path, mime: &str) -> Result<()> {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("opening {}", path.display()))?;
        let resp = self
            .http
            .post(format!(
                "{}/thumbnails/set?videoId={video_id}&uploadType=media",
                self.upload_base
            ))
            .bearer_auth(&self.token)
            .header(header::CONTENT_TYPE, mime)
            .body(bytes)
            .send()
            .await?;
        check(resp).await?;
        Ok(())
    }

    async fn add_to_playlist(&self, video_id: &str, playlist: &str) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/playlistItems?part=snippet", self.api_base))
            .bearer_auth(&self.token)
            .json(&json!({
                "snippet": {
                    "playlistId": playlist,
                    "resourceId": { "kind": "youtube#video", "videoId": video_id },
                }
            }))
            .send()
            .await?;
        check(resp).await?;
        Ok(())
    }
}

/// `Content-Range` header for a chunk of `len` bytes starting at `start`.
fn content_range(start: u64, len: usize, total: u64) -> String {
    format!("bytes {}-{}/{}", start, start + len as u64 - 1, total)
}

/// Fills `buf` from `file`, returning fewer bytes only at end of file.
async fn read_full(file: &mut tokio::fs::File, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = file.read(&mut buf[filled..]).await?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// An error response from the YouTube API. Keeps Google's reason code because the status alone
/// is ambiguous (a 403 can mean "not allowed" or "quota exceeded").
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub reason: Option<String>,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YouTube API error ({}): {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

/// Whether YouTube refused a thumbnail because the channel may not set custom thumbnails.
pub fn is_thumbnail_not_allowed(err: &anyhow::Error) -> bool {
    err.downcast_ref::<ApiError>().is_some_and(|e| {
        e.status == StatusCode::FORBIDDEN && e.reason.as_deref() == Some("forbidden")
    })
}

/// Turns an API error response into an [`ApiError`] with Google's message.
async fn check(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let error = serde_json::from_str::<Value>(&text)
        .ok()
        .map(|v| v["error"].clone())
        .unwrap_or_default();
    let reason = error["errors"][0]["reason"].as_str().map(str::to_owned);
    let message = error["message"].as_str().map(str::to_owned).unwrap_or(text);
    Err(ApiError {
        status,
        reason,
        message,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header as has_header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("yt-upload").chain(args.iter().copied())).unwrap()
    }

    fn upload_args(args: &[&str]) -> UploadArgs {
        let mut full = vec!["upload"];
        full.extend_from_slice(args);
        match parse(&full).command {
            Command::Upload(a) => *a,
            other => panic!("expected upload, got {other:?}"),
        }
    }

    async fn api(server: &MockServer, chunk: usize) -> Api {
        Api::with_endpoints("tok", &server.uri(), &server.uri(), chunk).unwrap()
    }

    fn temp_file(contents: &[u8], name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, contents).unwrap();
        (dir, path)
    }

    /// Mounts a resumable session whose upload URL is `<server>/session`.
    async fn mount_session(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/videos"))
            .and(query_param("uploadType", "resumable"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Location", format!("{}/session", server.uri()).as_str()),
            )
            .mount(server)
            .await;
    }

    #[test]
    fn parses_upload_with_all_options() {
        let a = upload_args(&[
            "v.mp4",
            "-t",
            "Title",
            "-d",
            "Desc",
            "--tags",
            "a, b,,c",
            "-p",
            "unlisted",
            "--category",
            "10",
            "--thumbnail",
            "t.png",
            "--playlist",
            "PL1",
            "--publish-at",
            "2026-10-25T15:00:00Z",
            "--made-for-kids",
        ]);
        assert_eq!(a.video, PathBuf::from("v.mp4"));
        assert_eq!(a.privacy, Privacy::Unlisted);
        assert_eq!(a.playlist.as_deref(), Some("PL1"));
        assert!(a.made_for_kids);
    }

    #[test]
    fn parses_other_commands_and_config_dir() {
        let cli = parse(&["--config-dir", "x", "auth"]);
        assert!(matches!(
            cli.command,
            Command::Auth(AuthArgs { account: None })
        ));
        assert!(matches!(
            parse(&["auth", "--account", "work"]).command,
            Command::Auth(a) if a.account.as_deref() == Some("work")
        ));
        assert!(matches!(
            parse(&["accounts", "list"]).command,
            Command::Accounts(AccountsCommand::List)
        ));
        assert!(matches!(
            parse(&["accounts", "remove", "work"]).command,
            Command::Accounts(AccountsCommand::Remove { alias }) if alias == "work"
        ));
        assert_eq!(cli.config_dir, Some(PathBuf::from("x")));
        assert!(matches!(parse(&["config-dir"]).command, Command::ConfigDir));
    }

    /// `parse_cli` with a fake OS environment.
    fn parse_with(args: &[&str], os_env: &[(&str, &str)]) -> Result<Cli, clap::Error> {
        let lookup = |name: &str| {
            os_env
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| Persisted {
                    value: (*v).to_owned(),
                    source: "/etc/environment",
                })
        };
        parse_cli(
            std::iter::once("yt-upload").chain(args.iter().copied()),
            lookup,
        )
    }

    fn upload_of(cli: Cli) -> UploadArgs {
        match cli.command {
            Command::Upload(a) => *a,
            other => panic!("expected upload, got {other:?}"),
        }
    }

    #[test]
    fn os_environment_fills_unset_options() {
        let cli = parse_with(
            &["upload", "v.mp4", "-t", "T"],
            &[
                (HOME_VAR, "os-home"),
                (PRIVACY_VAR, "Unlisted"),
                (CATEGORY_VAR, "10"),
                (TAGS_VAR, "a,b"),
                (PLAYLIST_VAR, "PL1"),
                (MADE_FOR_KIDS_VAR, "yes"),
                ("YT_UPLOAD_CLI_TITLE", "ignored"),
            ],
        )
        .unwrap();
        assert_eq!(cli.config_dir, Some(PathBuf::from("os-home")));
        let a = upload_of(cli);
        assert_eq!(a.title, "T");
        assert_eq!(a.privacy, Privacy::Unlisted);
        assert_eq!(a.category, "10");
        assert_eq!(a.tags, "a,b");
        assert_eq!(a.playlist.as_deref(), Some("PL1"));
        assert!(a.made_for_kids);
    }

    #[test]
    fn flags_beat_os_environment() {
        let cli = parse_with(
            &[
                "--config-dir",
                "flag-home",
                "upload",
                "v.mp4",
                "-t",
                "T",
                "-p",
                "public",
                "--category",
                "20",
                "--made-for-kids",
            ],
            &[
                (HOME_VAR, "os-home"),
                (PRIVACY_VAR, "secret"),
                (CATEGORY_VAR, "10"),
                (MADE_FOR_KIDS_VAR, "maybe"),
            ],
        )
        .unwrap();
        assert_eq!(cli.config_dir, Some(PathBuf::from("flag-home")));
        let a = upload_of(cli);
        assert_eq!(a.privacy, Privacy::Public);
        assert_eq!(a.category, "20");
        assert!(a.made_for_kids);
    }

    #[test]
    fn invalid_os_values_name_variable_and_source() {
        let err = parse_with(&["upload", "v.mp4", "-t", "T"], &[(PRIVACY_VAR, "secret")])
            .unwrap_err()
            .to_string();
        assert!(err.contains("'secret' for YT_UPLOAD_CLI_PRIVACY in the OS environment"));
        assert!(err.contains("(/etc/environment)"));
        assert!(err.contains("private, unlisted, public"));
        let err = parse_with(
            &["upload", "v.mp4", "-t", "T"],
            &[(MADE_FOR_KIDS_VAR, "maybe")],
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("YT_UPLOAD_CLI_MADE_FOR_KIDS"));
    }

    #[test]
    fn empty_os_values_and_defaults() {
        let a = upload_of(
            parse_with(
                &["upload", "v.mp4", "-t", "T"],
                &[(PRIVACY_VAR, ""), (MADE_FOR_KIDS_VAR, "off")],
            )
            .unwrap(),
        );
        assert_eq!(a.privacy, Privacy::Private);
        assert!(!a.made_for_kids);
    }

    #[test]
    fn account_comes_from_flag_or_os_environment() {
        let os = [(ACCOUNT_VAR, "os-acct")];
        let a = upload_of(parse_with(&["upload", "v.mp4", "-t", "T"], &os).unwrap());
        assert_eq!(a.account.as_deref(), Some("os-acct"));
        let a = upload_of(
            parse_with(&["upload", "v.mp4", "-t", "T", "--account", "flag"], &os).unwrap(),
        );
        assert_eq!(a.account.as_deref(), Some("flag"));
        match parse_with(&["auth"], &os).unwrap().command {
            Command::Auth(a) => assert_eq!(a.account.as_deref(), Some("os-acct")),
            other => panic!("expected auth, got {other:?}"),
        }
        match parse_with(&["auth", "--account", "flag"], &os)
            .unwrap()
            .command
        {
            Command::Auth(a) => assert_eq!(a.account.as_deref(), Some("flag")),
            other => panic!("expected auth, got {other:?}"),
        }
    }

    #[test]
    fn title_is_never_read_from_the_environment() {
        let err = parse_with(&["upload", "v.mp4"], &[("YT_UPLOAD_CLI_TITLE", "T")]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn other_commands_only_use_the_home_variable() {
        let cli = parse_with(&["config-dir"], &[(HOME_VAR, "os-home")]).unwrap();
        assert!(matches!(cli.command, Command::ConfigDir));
        assert_eq!(cli.config_dir, Some(PathBuf::from("os-home")));
    }

    #[test]
    fn parses_booleans() {
        for v in ["true", "1", "YES", "on"] {
            assert_eq!(parse_bool(v), Some(true));
        }
        for v in ["False", "0", "no", "OFF"] {
            assert_eq!(parse_bool(v), Some(false));
        }
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn privacy_strings() {
        assert_eq!(Privacy::Private.as_str(), "private");
        assert_eq!(Privacy::Unlisted.as_str(), "unlisted");
        assert_eq!(Privacy::Public.as_str(), "public");
    }

    #[test]
    fn config_dir_prefers_explicit_folder() {
        assert_eq!(
            config_dir(Some(PathBuf::from("x"))).unwrap(),
            PathBuf::from("x")
        );
        assert!(config_dir(None).unwrap().ends_with("yt-upload-cli"));
    }

    #[test]
    fn client_secret_path_requires_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = client_secret_path(dir.path()).unwrap_err().to_string();
        assert!(err.contains("Desktop OAuth client"));
        std::fs::write(dir.path().join("client_secret.json"), "{}").unwrap();
        assert!(client_secret_path(dir.path()).is_ok());
    }

    #[test]
    fn auth_messages() {
        let channel = Channel {
            id: "UC1".into(),
            title: "My Channel".into(),
            handle: None,
        };
        assert_eq!(auth_message(&channel), "Authorized. Channel: My Channel");
    }

    #[test]
    fn consent_url_shows_the_account_chooser() {
        assert_eq!(
            with_account_chooser("https://a/auth?scope=x"),
            "https://a/auth?scope=x&prompt=select_account%20consent"
        );
        assert_eq!(
            with_account_chooser("https://a/auth"),
            "https://a/auth?prompt=select_account%20consent"
        );
        let given = "https://a/auth?prompt=none&scope=x";
        assert_eq!(with_account_chooser(given), given);
    }

    #[test]
    fn video_body_maps_arguments() {
        let body = video_body(&upload_args(&[
            "v.mp4", "-t", "T", "--tags", " a, ,b ", "-p", "public",
        ]));
        assert_eq!(body["snippet"]["title"], "T");
        assert_eq!(body["snippet"]["tags"], json!(["a", "b"]));
        assert_eq!(body["snippet"]["categoryId"], "22");
        assert_eq!(body["status"]["privacyStatus"], "public");
        assert!(body["status"].get("publishAt").is_none());
    }

    #[test]
    fn scheduled_videos_are_private_until_published() {
        let body = video_body(&upload_args(&[
            "v.mp4",
            "-t",
            "T",
            "-p",
            "public",
            "--publish-at",
            "2026-10-25T15:00:00Z",
        ]));
        assert_eq!(body["status"]["privacyStatus"], "private");
        assert_eq!(body["status"]["publishAt"], "2026-10-25T15:00:00Z");
    }

    #[test]
    fn validates_thumbnails() {
        let (_d1, png) = temp_file(b"img", "a.PNG");
        assert_eq!(validate_thumbnail(&png).unwrap(), "image/png");
        let (_d2, jpeg) = temp_file(b"img", "a.jpeg");
        assert_eq!(validate_thumbnail(&jpeg).unwrap(), "image/jpeg");

        let err = |p: &Path| validate_thumbnail(p).unwrap_err().to_string();
        assert!(err(Path::new("missing.jpg")).contains("opening thumbnail missing.jpg"));
        let (_d3, webp) = temp_file(b"img", "a.webp");
        assert!(err(&webp).contains("must be a JPG or PNG"));
        assert!(err(Path::new("noext")).contains("must be a JPG or PNG"));
        let big = vec![0u8; MAX_THUMBNAIL_BYTES as usize + 1];
        let (_d4, big) = temp_file(&big, "big.jpg");
        assert!(err(&big).contains("exceeds 2 MB"));
        let max = vec![0u8; MAX_THUMBNAIL_BYTES as usize];
        let (_d5, max) = temp_file(&max, "max.jpg");
        assert!(validate_thumbnail(&max).is_ok());
    }

    #[test]
    fn thumbnail_refusal_is_told_apart_from_other_errors() {
        let api_err = |status: u16, reason: Option<&str>| {
            anyhow::Error::from(ApiError {
                status: StatusCode::from_u16(status).unwrap(),
                reason: reason.map(str::to_owned),
                message: "m".into(),
            })
        };
        assert!(is_thumbnail_not_allowed(&api_err(403, Some("forbidden"))));
        assert!(is_thumbnail_not_allowed(
            &api_err(403, Some("forbidden")).context("setting the thumbnail")
        ));
        assert!(!is_thumbnail_not_allowed(&api_err(
            403,
            Some("quotaExceeded")
        )));
        assert!(!is_thumbnail_not_allowed(&api_err(403, None)));
        assert!(!is_thumbnail_not_allowed(&api_err(500, Some("forbidden"))));
        assert!(!is_thumbnail_not_allowed(&anyhow::anyhow!("other")));
    }

    #[test]
    fn content_range_is_inclusive() {
        assert_eq!(content_range(0, 4, 10), "bytes 0-3/10");
        assert_eq!(content_range(8, 2, 10), "bytes 8-9/10");
    }

    #[test]
    fn production_client_builds() {
        assert!(Api::new("tok").is_ok());
    }

    #[tokio::test]
    async fn lists_channels() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels"))
            .and(has_header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    { "id": "UC1", "snippet": { "title": "My Channel", "customUrl": "@mine" } },
                    { "snippet": {} }
                ]
            })))
            .mount(&server)
            .await;
        let channels = api(&server, 4).await.channels().await.unwrap();
        assert_eq!(
            channels,
            [Channel {
                id: "UC1".into(),
                title: "My Channel".into(),
                handle: Some("@mine".into())
            }]
        );
    }

    #[tokio::test]
    async fn api_errors_show_googles_message() {
        let server = MockServer::start().await;
        Mock::given(path("/channels"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "error": { "message": "quota exceeded" }
            })))
            .mount(&server)
            .await;
        let err = api(&server, 4).await.channels().await.unwrap_err();
        assert!(err.to_string().contains("(403 Forbidden): quota exceeded"));
    }

    #[tokio::test]
    async fn api_errors_keep_googles_reason() {
        let server = MockServer::start().await;
        Mock::given(path("/channels"))
            .respond_with(ResponseTemplate::new(403).set_body_json(google_error(
                "quotaExceeded",
                "The request cannot be completed because you have exceeded your quota.",
            )))
            .mount(&server)
            .await;
        let err = api(&server, 4).await.channels().await.unwrap_err();
        let api_err = err.downcast_ref::<ApiError>().unwrap();
        assert_eq!(api_err.status, StatusCode::FORBIDDEN);
        assert_eq!(api_err.reason.as_deref(), Some("quotaExceeded"));
    }

    #[tokio::test]
    async fn api_errors_fall_back_to_raw_body() {
        let server = MockServer::start().await;
        Mock::given(path("/channels"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let err = api(&server, 4).await.channels().await.unwrap_err();
        assert!(err.to_string().ends_with(": boom"));
    }

    #[tokio::test]
    async fn uploads_in_chunks_then_sets_thumbnail_and_playlist() {
        let server = MockServer::start().await;
        mount_session(&server).await;
        Mock::given(method("PUT"))
            .and(path("/session"))
            .and(has_header("content-range", "bytes 0-3/6"))
            .respond_with(ResponseTemplate::new(308))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/session"))
            .and(has_header("content-range", "bytes 4-5/6"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "abc" })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/thumbnails/set"))
            .and(query_param("videoId", "abc"))
            .and(has_header("content-type", "image/png"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/playlistItems"))
            .and(body_partial_json(json!({
                "snippet": { "playlistId": "PL1", "resourceId": { "videoId": "abc" } }
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let (_dir, video) = temp_file(b"123456", "v.mp4");
        let (_tdir, thumb) = temp_file(b"img", "t.png");
        let args = upload_args(&[
            video.to_str().unwrap(),
            "-t",
            "T",
            "--thumbnail",
            thumb.to_str().unwrap(),
            "--playlist",
            "PL1",
        ]);
        assert_eq!(api(&server, 4).await.upload(&args).await.unwrap(), "abc");
    }

    #[tokio::test]
    async fn upload_rejects_missing_and_empty_files() {
        let server = MockServer::start().await;
        let api = api(&server, 4).await;
        let missing = upload_args(&["does-not-exist.mp4", "-t", "T"]);
        assert!(
            api.upload(&missing)
                .await
                .unwrap_err()
                .to_string()
                .contains("opening")
        );
        let (_dir, empty) = temp_file(b"", "empty.mp4");
        let args = upload_args(&[empty.to_str().unwrap(), "-t", "T"]);
        assert!(
            api.upload(&args)
                .await
                .unwrap_err()
                .to_string()
                .contains("is empty")
        );
    }

    #[tokio::test]
    async fn upload_fails_without_session_url() {
        let server = MockServer::start().await;
        Mock::given(path("/videos"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let (_dir, video) = temp_file(b"1", "v.mp4");
        let args = upload_args(&[video.to_str().unwrap(), "-t", "T"]);
        let err = api(&server, 4).await.upload(&args).await.unwrap_err();
        assert!(err.to_string().contains("upload URL"));
    }

    #[tokio::test]
    async fn upload_fails_without_video_id() {
        let server = MockServer::start().await;
        mount_session(&server).await;
        Mock::given(path("/session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let (_dir, video) = temp_file(b"1", "v.mp4");
        let args = upload_args(&[video.to_str().unwrap(), "-t", "T"]);
        let err = api(&server, 4).await.upload(&args).await.unwrap_err();
        assert!(err.to_string().contains("without a video id"));
    }

    #[tokio::test]
    async fn invalid_thumbnails_fail_before_uploading() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/videos"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let (_dir, video) = temp_file(b"1", "v.mp4");
        let (_tdir, webp) = temp_file(b"img", "t.webp");
        for thumb in ["missing.jpg", webp.to_str().unwrap()] {
            let args = upload_args(&[video.to_str().unwrap(), "-t", "T", "--thumbnail", thumb]);
            let err = api(&server, 4).await.upload(&args).await.unwrap_err();
            assert!(err.to_string().contains(thumb), "{err}");
        }
    }

    fn google_error(reason: &str, message: &str) -> Value {
        json!({ "error": { "message": message, "errors": [{ "reason": reason }] } })
    }

    /// Uploads a one-byte video with a thumbnail and `--playlist PL1`, where YouTube answers the
    /// thumbnail with `thumb` and the playlist insert with `playlist_status` (expected once).
    async fn upload_with_thumbnail(
        thumb: ResponseTemplate,
        playlist_status: u16,
    ) -> Result<String> {
        let server = MockServer::start().await;
        mount_session(&server).await;
        Mock::given(path("/session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "abc" })))
            .mount(&server)
            .await;
        Mock::given(path("/thumbnails/set"))
            .respond_with(thumb)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/playlistItems"))
            .respond_with(
                ResponseTemplate::new(playlist_status)
                    .set_body_json(google_error("playlistNotFound", "Playlist not found.")),
            )
            .expect(1)
            .mount(&server)
            .await;
        let (_dir, video) = temp_file(b"1", "v.mp4");
        let (_tdir, thumb) = temp_file(b"img", "t.jpg");
        let args = upload_args(&[
            video.to_str().unwrap(),
            "-t",
            "T",
            "--thumbnail",
            thumb.to_str().unwrap(),
            "--playlist",
            "PL1",
        ]);
        api(&server, 4).await.upload(&args).await
    }

    #[tokio::test]
    async fn thumbnail_not_allowed_is_skipped() {
        let refused = ResponseTemplate::new(403).set_body_json(google_error(
            "forbidden",
            "The authenticated user doesn't have permissions to upload and set custom video thumbnails.",
        ));
        assert_eq!(upload_with_thumbnail(refused, 200).await.unwrap(), "abc");
    }

    #[tokio::test]
    async fn other_thumbnail_errors_fail_after_the_playlist() {
        let quota = || {
            ResponseTemplate::new(403).set_body_json(google_error(
                "quotaExceeded",
                "You have exceeded your quota.",
            ))
        };
        let err = upload_with_thumbnail(quota(), 200).await.unwrap_err();
        assert!(
            format!("{err:#}").contains(
                "thumbnail: YouTube API error (403 Forbidden): You have exceeded your quota."
            ),
            "{err:#}"
        );

        let err = upload_with_thumbnail(quota(), 404).await.unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("Playlist not found.") && text.contains("exceeded your quota"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn playlist_errors_still_fail_the_upload() {
        let err = upload_with_thumbnail(ResponseTemplate::new(200), 404)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Playlist not found."));
    }
}
