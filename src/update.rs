use crate::model::AppResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MANIFEST_SCHEMA: u32 = 1;
pub const REPOSITORY: &str = "abrahamahn/process-optimizer";
pub const LATEST_MANIFEST_URL: &str =
    "https://github.com/abrahamahn/process-optimizer/releases/latest/download/update-manifest.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub app_url: String,
    pub app_sha256: String,
    pub updater_url: String,
    pub updater_sha256: String,
    pub installer_url: String,
}

fn version_parts(value: &str) -> AppResult<[u64; 3]> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let mut parts = value.split('.');
    let parse = |part: Option<&str>| -> AppResult<u64> {
        let part = part.ok_or("Version must contain major.minor.patch.")?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Version must contain only numeric major.minor.patch fields.".into());
        }
        part.parse::<u64>()
            .map_err(|_| "Version field is too large.".into())
    };
    let result = [
        parse(parts.next())?,
        parse(parts.next())?,
        parse(parts.next())?,
    ];
    if parts.next().is_some() {
        return Err("Only stable major.minor.patch releases are supported.".into());
    }
    Ok(result)
}

pub fn is_newer(current: &str, candidate: &str) -> AppResult<bool> {
    Ok(version_parts(candidate)? > version_parts(current)?)
}

fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Manifest {
    pub fn validate(&self) -> AppResult<()> {
        if self.schema != MANIFEST_SCHEMA {
            return Err("Unsupported update manifest schema.".into());
        }
        version_parts(&self.version)?;
        if !valid_sha(&self.app_sha256) || !valid_sha(&self.updater_sha256) {
            return Err("Update manifest contains an invalid SHA-256 digest.".into());
        }
        let tag = format!("v{}", self.version);
        let base = format!("https://github.com/{REPOSITORY}/releases/download/{tag}/");
        if self.app_url != format!("{base}process-optimizer.exe")
            || self.updater_url != format!("{base}process-optimizer-updater.exe")
            || self.installer_url != format!("{base}ProcessOptimizerSetup.exe")
        {
            return Err(
                "Update manifest points outside the expected GitHub release assets.".into(),
            );
        }
        Ok(())
    }
}

pub fn parse_manifest(bytes: &[u8]) -> AppResult<Manifest> {
    if bytes.len() > 256 * 1024 {
        return Err("Update manifest is unexpectedly large.".into());
    }
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|e| format!("Could not parse update manifest: {e}"))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(sha256_bytes(&bytes))
}

pub fn verify_file(path: &Path, expected: &str) -> AppResult<()> {
    if !valid_sha(expected) {
        return Err("Expected update digest is invalid.".into());
    }
    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "Downloaded update failed SHA-256 verification. Expected {expected}, got {actual}."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        let base = format!("https://github.com/{REPOSITORY}/releases/download/v{version}/");
        Manifest {
            schema: MANIFEST_SCHEMA,
            version: version.into(),
            app_url: format!("{base}process-optimizer.exe"),
            app_sha256: "a".repeat(64),
            updater_url: format!("{base}process-optimizer-updater.exe"),
            updater_sha256: "b".repeat(64),
            installer_url: format!("{base}ProcessOptimizerSetup.exe"),
        }
    }

    #[test]
    fn stable_versions_compare_numerically() {
        assert!(is_newer("0.4.9", "0.5.0").unwrap());
        assert!(is_newer("0.9.9", "0.10.0").unwrap());
        assert!(!is_newer("1.2.3", "1.2.3").unwrap());
        assert!(!is_newer("2.0.0", "1.99.99").unwrap());
    }

    #[test]
    fn manifest_is_repository_and_asset_scoped() {
        let mut m = manifest("0.5.0");
        m.validate().unwrap();
        m.app_url = "https://example.com/process-optimizer.exe".into();
        assert!(m.validate().is_err());
    }

    #[test]
    fn malformed_versions_and_hashes_fail_closed() {
        assert!(is_newer("0.4.0", "0.5-beta").is_err());
        let mut m = manifest("0.5.0");
        m.app_sha256 = "not-a-hash".into();
        assert!(m.validate().is_err());
    }
}
