use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

const ENDPOINT: &str =
    "https://raw.githubusercontent.com/MatilyLabs/MaxRAR/main/promotions/feed.json";
const DAY: u64 = 86_400;
const MAX_BYTES: usize = 65_536;
#[derive(Default)]
pub struct Promotions(Mutex<()>);
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Campaign {
    id: String,
    title: String,
    description: String,
    button: String,
    url: String,
    icon: String,
    starts_at: Option<u64>,
    ends_at: Option<u64>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feed {
    schema_version: u8,
    enabled: bool,
    valid_until: u64,
    rotation_seconds: u64,
    campaigns: Vec<Campaign>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}
#[derive(Default, Deserialize, Serialize)]
struct Cache {
    checked_at: u64,
    envelope: Option<Envelope>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn key() -> Option<&'static str> {
    option_env!("MAXRAR_PROMOTIONS_PUBLIC_KEY")
        .map(str::trim)
        .filter(|s| !s.is_empty())
}
fn allowed_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && (url.host_str() == Some("www.matily.org")
            || (url.host_str() == Some("github.com")
                && (url.path() == "/MatilyLabs" || url.path().starts_with("/MatilyLabs/"))))
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && value.len() <= 512
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}
fn validate(feed: &Feed, time: u64) -> Result<(), String> {
    if feed.schema_version != 1
        || feed.valid_until <= time
        || feed.valid_until > time.saturating_add(31 * DAY)
        || !(10..=120).contains(&feed.rotation_seconds)
        || feed.campaigns.len() > 8
    {
        return Err("Invalid promotion feed".into());
    }
    let mut ids = std::collections::HashSet::new();
    for c in &feed.campaigns {
        if c.id.is_empty()
            || c.id.len() > 64
            || !c
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&c.id)
            || !text(&c.title, 64)
            || !text(&c.description, 160)
            || !text(&c.button, 28)
            || !allowed_url(&c.url)
            || !["app", "scan", "archive"].contains(&c.icon.as_str())
            || c.starts_at
                .zip(c.ends_at)
                .is_some_and(|(start, end)| start >= end)
        {
            return Err("Invalid promotion campaign".into());
        }
    }
    Ok(())
}
fn verified(envelope: &Envelope, public_key: &str, time: u64) -> Result<Feed, String> {
    let public_key = PublicKey::from_base64(public_key).map_err(|_| "Invalid promotion key")?;
    let signature =
        Signature::decode(&envelope.signature).map_err(|_| "Invalid promotion signature")?;
    public_key
        .verify(envelope.payload.as_bytes(), &signature, false)
        .map_err(|_| "Promotion signature rejected")?;
    let feed: Feed =
        serde_json::from_str(&envelope.payload).map_err(|_| "Invalid promotion JSON")?;
    validate(&feed, time)?;
    Ok(feed)
}
fn active(c: &Campaign, time: u64) -> bool {
    c.starts_at.is_none_or(|start| time >= start) && c.ends_at.is_none_or(|end| time < end)
}
fn read_cache(app: &AppHandle) -> Cache {
    app.path()
        .app_cache_dir()
        .ok()
        .and_then(|p| {
            let p = p.join("promotions.json");
            if std::fs::metadata(&p).ok()?.len() > (MAX_BYTES * 2) as u64 {
                return None;
            }
            serde_json::from_slice(&std::fs::read(p).ok()?).ok()
        })
        .unwrap_or_default()
}
fn write_cache(app: &AppHandle, cache: &Cache) {
    if let Ok(dir) = app.path().app_cache_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(bytes) = serde_json::to_vec(cache) {
                let temp = dir.join("promotions.tmp");
                if std::fs::write(&temp, bytes).is_ok() {
                    let _ = std::fs::rename(temp, dir.join("promotions.json"));
                }
            }
        }
    }
}
async fn fetch() -> Result<Envelope, String> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(ENDPOINT)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("Promotion feed too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn promotion_feed(
    app: AppHandle,
    state: State<'_, Promotions>,
) -> Result<Option<Feed>, String> {
    let Some(public_key) = key() else {
        return Ok(None);
    };
    let _guard = state.0.lock().await;
    let time = now();
    let mut cache = read_cache(&app);
    if cache.checked_at > time || time.saturating_sub(cache.checked_at) >= DAY {
        cache.checked_at = time;
        if let Ok(envelope) = fetch().await {
            if verified(&envelope, public_key, time).is_ok() {
                cache.envelope = Some(envelope);
            }
        }
        write_cache(&app, &cache);
    }
    Ok(cache
        .envelope
        .as_ref()
        .and_then(|e| verified(e, public_key, time).ok()))
}
#[tauri::command]
pub async fn open_promotion(
    app: AppHandle,
    campaign_id: String,
    state: State<'_, Promotions>,
) -> Result<(), String> {
    let public_key = key().ok_or("Promotions are disabled")?;
    let _guard = state.0.lock().await;
    let time = now();
    let cache = read_cache(&app);
    let feed = verified(
        cache.envelope.as_ref().ok_or("Campaign unavailable")?,
        public_key,
        time,
    )?;
    let campaign = feed
        .campaigns
        .iter()
        .find(|c| feed.enabled && c.id == campaign_id && active(c, time))
        .ok_or("Campaign unavailable")?;
    app.opener()
        .open_url(&campaign.url, None::<&str>)
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn feed() -> Feed {
        serde_json::from_str(r#"{"schemaVersion":1,"enabled":true,"validUntil":200000,"rotationSeconds":10,"campaigns":[{"id":"launch","title":"Product","description":"Learn more.","button":"Learn more","url":"https://github.com/MatilyLabs/MaxRAR","icon":"app","startsAt":null,"endsAt":null}]}"#).unwrap()
    }
    #[test]
    fn rejects_destinations_and_expired_content() {
        for url in [
            "http://github.com/MatilyLabs",
            "https://github.com.evil.test/MatilyLabs",
            "https://github.com/Other",
            "https://user@github.com/MatilyLabs",
            "https://github.com/MatilyLabs?tracking=1",
            "javascript:alert(1)",
            "https://www.matily.org.evil.test/",
            "https://www.matily.org@evil.test/",
            "http://www.matily.org/",
        ] {
            assert!(!allowed_url(url), "{url}");
        }
        assert!(allowed_url("https://www.matily.org"));
        assert!(allowed_url("https://www.matily.org/products"));
        assert!(validate(&feed(), 100000).is_ok());
        assert!(validate(&feed(), 200000).is_err());
        let mut f = feed();
        f.campaigns.push(f.campaigns[0].clone());
        assert!(validate(&f, 100000).is_err());
    }
    #[test]
    fn accepts_signed_feed_and_rejects_tampering() {
        let envelope: Envelope = serde_json::from_str(include_str!(
            "../../verification/fixtures/promotions/valid-envelope.json"
        ))
        .unwrap();
        let public_key = include_str!("../../verification/fixtures/promotions/test-public-key.txt");
        assert!(verified(&envelope, public_key, 100000).is_ok());
        assert!(verified(&envelope, public_key, 200000).is_err());
        let tampered = Envelope {
            payload: envelope
                .payload
                .replace("Test campaign", "Changed campaign"),
            signature: envelope.signature,
        };
        assert!(verified(&tampered, public_key, 100000).is_err());
    }
    #[test]
    fn unsigned_feed_is_rejected() {
        assert!(verified(
            &Envelope {
                payload: "{}".into(),
                signature: "invalid".into()
            },
            "invalid",
            100000
        )
        .is_err());
    }
}
