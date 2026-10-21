//! Library behind the `yt-upload` binary: the CLI definition and the YouTube Data API v3 calls.
//! The interactive OAuth flow lives in the binary (`src/oauth.rs`); everything here is testable
//! against a mock server.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
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
#[command(version, about = "Upload videos to YouTube from the command line")]
pub struct Cli {
    /// Folder holding client_secret.json and the saved token
    /// [default: <config dir>/yt-upload-cli]
    #[arg(long, env = "YT_UPLOAD_CLI_HOME", global = true)]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Authorize a YouTube account and show the connected channel
    Auth,
    /// Upload a video
    Upload(Box<UploadArgs>),
    /// Print the config folder path
    ConfigDir,
}

#[derive(clap::Args, Debug)]
pub struct UploadArgs {
    /// Video file to upload
    pub video: PathBuf,
    #[arg(short, long)]
    pub title: String,
    #[arg(short, long, default_value = "")]
    pub description: String,
    /// Comma-separated tags
    #[arg(long, default_value = "")]
    pub tags: String,
    #[arg(short, long, value_enum, default_value_t = Privacy::Private)]
    pub privacy: Privacy,
    /// Category id (22 = People & Blogs)
    #[arg(long, default_value = "22")]
    pub category: String,
    /// Thumbnail image (JPG or PNG, up to 2 MB)
    #[arg(long)]
    pub thumbnail: Option<PathBuf>,
    /// Playlist id to add the video to
    #[arg(long)]
    pub playlist: Option<String>,
    /// Schedule publishing (RFC 3339, e.g. 2026-10-25T15:00:00Z); forces private until then
    #[arg(long)]
    pub publish_at: Option<String>,
    /// Mark the video as made for kids
    #[arg(long)]
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

/// Message shown by `yt-upload auth` for the channels the token can access.
pub fn auth_message(channels: &[String]) -> String {
    if channels.is_empty() {
        "Authorized, but this account has no YouTube channel.".to_owned()
    } else {
        format!("Authorized. Channel: {}", channels.join(", "))
    }
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

/// MIME type for a thumbnail file, based on its extension.
pub fn thumbnail_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        _ => "image/jpeg",
    }
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

    /// Names of the channels owned by the authorized account.
    pub async fn channel_names(&self) -> Result<Vec<String>> {
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
            .filter_map(|c| c["snippet"]["title"].as_str().map(str::to_owned))
            .collect())
    }

    /// Uploads the video, then sets the thumbnail and playlist if requested.
    /// Returns the new video id. Progress goes to stderr.
    pub async fn upload(&self, args: &UploadArgs) -> Result<String> {
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

        if let Some(thumb) = &args.thumbnail {
            self.set_thumbnail(&id, thumb).await?;
            println!("Thumbnail set.");
        }
        if let Some(playlist) = &args.playlist {
            self.add_to_playlist(&id, playlist).await?;
            println!("Added to playlist.");
        }
        Ok(id)
    }

    async fn set_thumbnail(&self, video_id: &str, path: &Path) -> Result<()> {
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
            .header(header::CONTENT_TYPE, thumbnail_mime(path))
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
        assert!(matches!(cli.command, Command::Auth));
        assert_eq!(cli.config_dir, Some(PathBuf::from("x")));
        assert!(matches!(parse(&["config-dir"]).command, Command::ConfigDir));
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
        assert!(auth_message(&[]).contains("no YouTube channel"));
        assert_eq!(
            auth_message(&["A".into(), "B".into()]),
            "Authorized. Channel: A, B"
        );
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
    fn thumbnail_mime_by_extension() {
        assert_eq!(thumbnail_mime(Path::new("a.PNG")), "image/png");
        assert_eq!(thumbnail_mime(Path::new("a.jpg")), "image/jpeg");
        assert_eq!(thumbnail_mime(Path::new("noext")), "image/jpeg");
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
    async fn lists_channel_names() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels"))
            .and(has_header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [{ "snippet": { "title": "My Channel" } }, { "snippet": {} }]
            })))
            .mount(&server)
            .await;
        let names = api(&server, 4).await.channel_names().await.unwrap();
        assert_eq!(names, vec!["My Channel"]);
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
        let err = api(&server, 4).await.channel_names().await.unwrap_err();
        assert!(err.to_string().contains("(403 Forbidden): quota exceeded"));
    }

    #[tokio::test]
    async fn api_errors_fall_back_to_raw_body() {
        let server = MockServer::start().await;
        Mock::given(path("/channels"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let err = api(&server, 4).await.channel_names().await.unwrap_err();
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
    async fn upload_reports_missing_thumbnail_file() {
        let server = MockServer::start().await;
        mount_session(&server).await;
        Mock::given(path("/session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "abc" })))
            .mount(&server)
            .await;
        let (_dir, video) = temp_file(b"1", "v.mp4");
        let args = upload_args(&[
            video.to_str().unwrap(),
            "-t",
            "T",
            "--thumbnail",
            "missing.jpg",
        ]);
        let err = api(&server, 4).await.upload(&args).await.unwrap_err();
        assert!(err.to_string().contains("missing.jpg"));
    }
}
