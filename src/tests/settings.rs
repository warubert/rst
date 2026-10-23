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
