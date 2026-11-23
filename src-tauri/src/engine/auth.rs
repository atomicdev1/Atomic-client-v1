use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub xuid: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_user_type")]
    pub user_type: String,
}

fn default_user_type() -> String {
    "msa".to_string()
}

impl Account {
    pub fn is_fresh(&self) -> bool {
        self.expires_at > now_secs() + 120
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub interval: u64,
}

#[derive(Deserialize)]
struct MsaDeviceResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    interval: u64,
}

#[derive(Deserialize, Default)]
struct MsaTokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

#[derive(Deserialize, Default)]
struct XblResponse {
    #[serde(default, rename = "Token")]
    token: String,
    #[serde(default)]
    display_claims: serde_json::Value,
}

pub enum PollOutcome {
    Pending,
    Done(Account),
}

pub async fn begin_device_code(client: &reqwest::Client, client_id: &str) -> Result<DeviceCode> {
    if client_id.trim().is_empty() {
        return Err(anyhow!(
            "Set an Azure application (client) id in Settings first - see README"
        ));
    }
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", client_id),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let dc: MsaDeviceResponse = res.json().await?;
    Ok(DeviceCode {
        device_code: dc.device_code,
        user_code: dc.user_code,
        verification_uri: dc.verification_uri,
        message: dc.message,
        expires_in: dc.expires_in,
        interval: dc.interval.max(2),
    })
}

pub async fn poll_device_code(
    client: &reqwest::Client,
    client_id: &str,
    device_code: &str,
) -> Result<PollOutcome> {
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", device_code),
        ])
        .send()
        .await?;
    let token: MsaTokenResponse = res.json().await.unwrap_or_default();
    if token.access_token.is_empty() {
        if token.error == "authorization_pending" || token.error == "slow_down" {
            return Ok(PollOutcome::Pending);
        }
        return Err(anyhow!(
            "Microsoft login: {} {}",
            token.error,
            token.error_description
        ));
    }
    let account = finish_login(
        client,
        client_id,
        &token.access_token,
        &token.refresh_token,
        token.expires_in,
    )
    .await?;
    Ok(PollOutcome::Done(account))
}

pub async fn refresh(
    client: &reqwest::Client,
    client_id: &str,
    refresh_token: &str,
) -> Result<Account> {
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;
    let token: MsaTokenResponse = res.json().await.unwrap_or_default();
    if token.access_token.is_empty() {
        return Err(anyhow!("refresh failed: {}", token.error));
    }
    finish_login(
        client,
        client_id,
        &token.access_token,
        &token.refresh_token,
        token.expires_in,
    )
    .await
}

async fn finish_login(
    client: &reqwest::Client,
    client_id: &str,
    msa_token: &str,
    refresh_token: &str,
    expires_in: u64,
) -> Result<Account> {
    // 1) Xbox Live
    let xbl: XblResponse = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={msa_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Xbox Live authentication failed")?;
    let uhs = xbl
        .display_claims
        .get("xui")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    // 2) XSTS
    let res = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    if !res.status().is_success() {
        let body: serde_json::Value = res.json().await.unwrap_or_default();
        let xerr = body.get("XErr").and_then(|v| v.as_u64()).unwrap_or(0);
        let hint = match xerr {
            2148916233 => "This Microsoft account has no Xbox profile yet - sign in at xbox.com once, then retry.",
            2148916235 => "Xbox Live is not available in this account's country.",
            2148916236 | 2148916237 => "Adult verification is required for this account.",
            2148916238 => "This is a child account - add it to a Microsoft family first.",
            _ => "XSTS authorization failed.",
        };
        return Err(anyhow!("{hint} (XErr {xerr})"));
    }
    let xsts: XblResponse = res.json().await?;

    // 3) Minecraft services
    let mc: serde_json::Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token)
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Minecraft login failed")?;
    let mc_token = mc
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if mc_token.is_empty() {
        return Err(anyhow!("Minecraft login returned no access token"));
    }
    let mc_expires = mc
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(expires_in);

    // 4) Profile
    let pr = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_token)
        .send()
        .await?;
    if !pr.status().is_success() {
        return Err(anyhow!(
            "This Microsoft account does not own Minecraft: Java Edition."
        ));
    }
    let profile: serde_json::Value = pr.json().await?;
    let name = profile
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Player")
        .to_string();
    let uuid = profile
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Ok(Account {
        name,
        uuid,
        access_token: mc_token,
        refresh_token: refresh_token.to_string(),
        expires_at: now_secs() + mc_expires,
        xuid: uhs,
        client_id: client_id.to_string(),
        user_type: "msa".to_string(),
    })
}

pub fn offline_account(name: &str) -> Account {
    let digest = md5::compute(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = digest.0;
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    Account {
        name: name.to_string(),
        uuid,
        access_token: "0".to_string(),
        refresh_token: String::new(),
        expires_at: 0,
        xuid: String::new(),
        client_id: String::new(),
        user_type: "legacy".to_string(),
    }
}

pub fn save_account(dirs: &crate::engine::paths::Dirs, acc: &Account) -> Result<()> {
    let p = dirs.root.join("auth.json");
    std::fs::write(&p, serde_json::to_string_pretty(acc)?)?;
    Ok(())
}

pub fn load_account(dirs: &crate::engine::paths::Dirs) -> Option<Account> {
    let p = dirs.root.join("auth.json");
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}
