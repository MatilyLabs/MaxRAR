//! Release-only check: verify staged packages with the same Minisign verifier as the updater.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::{error::Error, fs, path::Path};

fn decode(text: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(STANDARD.decode(text.trim())?)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("Pass the staged release directory")?;
    let root = Path::new(&root);
    let config: Value = serde_json::from_str(include_str!("../../release.config.json"))?;
    let key = PublicKey::decode(&decode(
        config["updaterPublicKey"].as_str().ok_or("No key")?,
    )?)?;
    let mut count = 0;
    for file in fs::read_dir(root)? {
        let path = file?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let version = manifest["version"].as_str().ok_or("No version")?;
        for (platform, entry) in manifest["platforms"].as_object().ok_or("No platforms")? {
            let url = reqwest::Url::parse(entry["url"].as_str().ok_or("No URL")?)?;
            let name = url
                .path_segments()
                .and_then(|mut s| s.next_back())
                .ok_or("No filename")?;
            let bytes = fs::read(root.join(name))?;
            let signature =
                Signature::decode(&decode(entry["signature"].as_str().ok_or("No signature")?)?)?;
            key.verify(&bytes, &signature, true)?;
            let signed = signature
                .trusted_comment()
                .split('\t')
                .find_map(|s| s.strip_prefix("version:"));
            if signed != Some(version) {
                return Err("Signed version differs from manifest".into());
            }
            let mut changed = bytes;
            changed.push(0);
            if key.verify(&changed, &signature, true).is_ok() {
                return Err("Tampered package accepted".into());
            }
            println!("{platform}: signature, signed version and tamper rejection verified");
            count += 1;
        }
    }
    if count == 0 {
        return Err("No platform manifests verified".into());
    }
    Ok(())
}
