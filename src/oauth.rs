//! Interactive OAuth flow: opens the browser and stores the token in the account's folder.
//! Excluded from coverage because it needs a real Google account and a browser.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use anyhow::{Context, Result, anyhow};
use yt_upload_cli::{SCOPES, client_secret_path, with_account_chooser};
use yup_oauth2::authenticator_delegate::InstalledFlowDelegate;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

/// Opens the consent page in the default browser instead of only printing it.
struct BrowserDelegate;

impl InstalledFlowDelegate for BrowserDelegate {
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        _need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let url = with_account_chooser(url);
            eprintln!("Opening your browser to authorize. If it does not open, visit:\n{url}\n");
            let _ = webbrowser::open(&url);
            Ok(String::new())
        })
    }
}

/// A valid access token for the account whose token lives at `token_path`, running the browser
/// flow when there is no usable saved token.
pub async fn access_token(dir: &Path, token_path: &Path) -> Result<String> {
    let secret_path = client_secret_path(dir)?;
    let secret = yup_oauth2::read_application_secret(&secret_path)
        .await
        .with_context(|| format!("reading {}", secret_path.display()))?;
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
        .persist_tokens_to_disk(token_path)
        .flow_delegate(Box::new(BrowserDelegate))
        .build()
        .await?;
    let token = auth.token(SCOPES).await?;
    token
        .token()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("Google returned no access token"))
}
