use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const API_KEY_ENV: &str = "GOOGLE_TRANSLATE_API_KEY";

pub fn translation_api_key() -> Option<String> {
    let saved = api_key_path().and_then(|path| read_api_key_from(&path));
    resolve_api_key(api_key_enabled(), saved, env::var(API_KEY_ENV).ok())
}

pub fn has_configured_api_key() -> bool {
    api_key_path()
        .and_then(|path| read_api_key_from(&path))
        .is_some()
        || env::var(API_KEY_ENV)
            .ok()
            .is_some_and(|key| !key.trim().is_empty())
}

pub fn api_key_enabled() -> bool {
    api_key_enabled_from(setting_path("use_api_key").and_then(|path| fs::read_to_string(path).ok()))
}

fn api_key_enabled_from(value: Option<String>) -> bool {
    value.is_none_or(|value| value.trim() != "false")
}

pub fn set_api_key_enabled(enabled: bool) -> io::Result<()> {
    write_setting(
        &setting_path("use_api_key").ok_or_else(config_dir_error)?,
        if enabled { "true" } else { "false" },
    )
}

pub fn save_api_key(key: &str) -> io::Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "API key cannot be empty",
        ));
    }
    write_private_setting(&api_key_path().ok_or_else(config_dir_error)?, key)
}

pub fn clear_saved_api_key() -> io::Result<()> {
    match api_key_path() {
        Some(path) => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
        None => Err(config_dir_error()),
    }
}

fn resolve_api_key(
    enabled: bool,
    saved: Option<String>,
    environment: Option<String>,
) -> Option<String> {
    if !enabled {
        return None;
    }

    saved
        .or(environment)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn read_api_key_from(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn api_key_path() -> Option<PathBuf> {
    setting_path("google_translate_api_key")
}

fn setting_path(name: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("rst").join(name))
}

fn write_setting(path: &Path, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, value)
}

fn write_private_setting(path: &Path, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);

    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(value.as_bytes())?;
    file.flush()
}

fn config_dir_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        "user config directory is unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::{api_key_enabled_from, resolve_api_key, write_private_setting};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rst-settings-test-{unique}/api_key"))
    }

    #[test]
    fn key_toggle_selects_saved_key_environment_or_keyless_mode() {
        assert_eq!(
            resolve_api_key(true, Some("saved".to_string()), Some("env".to_string())).as_deref(),
            Some("saved")
        );
        assert_eq!(
            resolve_api_key(true, None, Some(" env ".to_string())).as_deref(),
            Some("env")
        );
        assert_eq!(
            resolve_api_key(false, Some("saved".to_string()), Some("env".to_string())),
            None
        );
        assert_eq!(resolve_api_key(true, None, None), None);
        assert_eq!(resolve_api_key(true, None, Some("  ".to_string())), None);
    }

    #[test]
    fn key_toggle_defaults_to_on_and_reads_saved_off_state() {
        assert!(api_key_enabled_from(None));
        assert!(api_key_enabled_from(Some("true".to_string())));
        assert!(!api_key_enabled_from(Some("false\n".to_string())));
    }

    #[test]
    fn api_key_is_saved_with_unix_private_permissions() {
        let path = temp_path();
        write_private_setting(&path, "secret-value").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "secret-value");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
