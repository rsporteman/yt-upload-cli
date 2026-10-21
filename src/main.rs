//! yt-upload-cli: upload videos to YouTube from the command line (YouTube Data API v3).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use yup_oauth2::authenticator_delegate::InstalledFlowDelegate;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/youtube.upload",
    "https://www.googleapis.com/auth/youtube",
];
const API: &str = "https://www.googleapis.com/youtube/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/youtube/v3";
/// Resumable upload chunks must be a multiple of 256 KiB.
const CHUNK_SIZE: usize = 32 * 256 * 1024;

#[derive(Parser)]
#[command(version, about = "Upload videos to YouTube from the command line")]
struct Cli {
    /// Folder holding client_secret.json and the saved token
    /// [default: <config dir>/yt-upload-cli]
    #[arg(long, env = "YT_UPLOAD_CLI_HOME", global = true)]
    config_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authorize a YouTube account and show the connected channel
    Auth,
    /// Upload a video
    Upload(Box<UploadArgs>),
    /// Print the config folder path
    ConfigDir,
}

#[derive(clap::Args)]
struct UploadArgs {
    /// Video file to upload
    video: PathBuf,
    #[arg(short, long)]
    title: String,
    #[arg(short, long, default_value = "")]
    description: String,
    /// Comma-separated tags
    #[arg(long, default_value = "")]
    tags: String,
    #[arg(short, long, value_enum, default_value_t = Privacy::Private)]
    privacy: Privacy,
    /// Category id (22 = People & Blogs)
    #[arg(long, default_value = "22")]
    category: String,
    /// Thumbnail image (JPG or PNG, up to 2 MB)
    #[arg(long)]
    thumbnail: Option<PathBuf>,
    /// Playlist id to add the video to
    #[arg(long)]
    playlist: Option<String>,
    /// Schedule publishing (RFC 3339, e.g. 2026-10-25T15:00:00Z); forces private until then
    #[arg(long)]
    publish_at: Option<String>,
    /// Mark the video as made for kids
    #[arg(long)]
    made_for_kids: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Privacy {
    Private,
    Unlisted,
    Public,
}

impl Privacy {
    fn as_str(self) -> &'static str {
        match self {
            Privacy::Private => "private",
            Privacy::Unlisted => "unlisted",
            Privacy::Public => "public",
        }
    }
}

/// Opens the consent page in the default browser instead of only printing it.
struct BrowserDelegate;

impl InstalledFlowDelegate for BrowserDelegate {
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        _need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            eprintln!("Opening your browser to authorize. If it does not open, visit:\n{url}\n");
            let _ = webbrowser::open(url);
            Ok(String::new())
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = match cli.config_dir {
        Some(d) => d,
        None => dirs::config_dir()
            .context("could not find the user config folder; use --config-dir")?
            .join("yt-upload-cli"),
    };

    match cli.command {
        Command::ConfigDir => {
            println!("{}", dir.display());
            Ok(())
        }
        Command::Auth => {
            let token = access_token(&dir).await?;
            let http = Client::new();
            let channels: Value = http
                .get(format!("{API}/channels?part=snippet&mine=true"))
                .bearer_auth(&token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let names: Vec<&str> = channels["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["snippet"]["title"].as_str())
                .collect();
            if names.is_empty() {
                println!("Authorized, but this account has no YouTube channel.");
            } else {
                println!("Authorized. Channel: {}", names.join(", "));
            }
            Ok(())
        }
        Command::Upload(args) => {
            let token = access_token(&dir).await?;
            upload(&token, *args).await
        }
    }
}

async fn access_token(dir: &Path) -> Result<String> {
    let secret_path = dir.join("client_secret.json");
    if !secret_path.exists() {
        bail!(
            "missing {}\nCreate a Desktop OAuth client in Google Cloud and save its JSON there.",
            secret_path.display()
        );
    }
    tokio::fs::create_dir_all(dir).await?;
    let secret = yup_oauth2::read_application_secret(&secret_path)
        .await
        .with_context(|| format!("reading {}", secret_path.display()))?;
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
        .persist_tokens_to_disk(dir.join("token.json"))
        .flow_delegate(Box::new(BrowserDelegate))
        .build()
        .await?;
    let token = auth.token(SCOPES).await?;
    token
        .token()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("Google returned no access token"))
}

async fn upload(token: &str, args: UploadArgs) -> Result<()> {
    // YouTube answers 308 "Resume Incomplete" between chunks; it must not be followed.
    let http = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut file = tokio::fs::File::open(&args.video)
        .await
        .with_context(|| format!("opening {}", args.video.display()))?;
    let total = file.metadata().await?.len();

    let mut status = json!({
        "privacyStatus": args.privacy.as_str(),
        "selfDeclaredMadeForKids": args.made_for_kids,
    });
    if let Some(at) = &args.publish_at {
        status["privacyStatus"] = json!("private");
        status["publishAt"] = json!(at);
    }
    let tags: Vec<&str> = args
        .tags
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    let body = json!({
        "snippet": {
            "title": args.title,
            "description": args.description,
            "tags": tags,
            "categoryId": args.category,
        },
        "status": status,
    });

    // Start a resumable session; Google answers with the upload URL in Location.
    let start = http
        .post(format!(
            "{UPLOAD_API}/videos?uploadType=resumable&part=snippet,status"
        ))
        .bearer_auth(token)
        .header("X-Upload-Content-Length", total)
        .header("X-Upload-Content-Type", "video/*")
        .json(&body)
        .send()
        .await?;
    let start = check(start).await?;
    let session = start
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .context("Google did not return an upload URL")?
        .to_owned();

    let mut sent: u64 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let video: Value = loop {
        let n = read_full(&mut file, &mut buf).await?;
        let end = sent + n as u64;
        let resp = http
            .put(&session)
            .bearer_auth(token)
            .header(header::CONTENT_LENGTH, n)
            .header(
                header::CONTENT_RANGE,
                format!("bytes {}-{}/{}", sent, end.saturating_sub(1), total),
            )
            .body(buf[..n].to_vec())
            .send()
            .await?;
        if resp.status() == StatusCode::PERMANENT_REDIRECT {
            sent = end;
            eprintln!("Uploading... {}%", sent * 100 / total.max(1));
            continue;
        }
        break check(resp).await?.json().await?;
    };

    let id = video["id"]
        .as_str()
        .context("upload finished without a video id")?;
    println!("Uploaded: https://youtu.be/{id}");

    if let Some(thumb) = &args.thumbnail {
        let bytes = tokio::fs::read(thumb)
            .await
            .with_context(|| format!("opening {}", thumb.display()))?;
        let mime = match thumb
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => "image/png",
            _ => "image/jpeg",
        };
        let resp = http
            .post(format!(
                "{UPLOAD_API}/thumbnails/set?videoId={id}&uploadType=media"
            ))
            .bearer_auth(token)
            .header(header::CONTENT_TYPE, mime)
            .body(bytes)
            .send()
            .await?;
        check(resp).await?;
        println!("Thumbnail set.");
    }

    if let Some(playlist) = &args.playlist {
        let resp = http
            .post(format!("{API}/playlistItems?part=snippet"))
            .bearer_auth(token)
            .json(&json!({
                "snippet": {
                    "playlistId": playlist,
                    "resourceId": { "kind": "youtube#video", "videoId": id },
                }
            }))
            .send()
            .await?;
        check(resp).await?;
        println!("Added to playlist.");
    }
    Ok(())
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

/// Turns an API error response into a readable error with Google's message.
async fn check(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or(text);
    bail!("YouTube API error ({status}): {message}")
}
