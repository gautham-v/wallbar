//! Start at login, via `SMAppService.mainApp` (macOS 13+), as the sibling
//! menu bar apps do.
//!
//! The popover has no settings section to hang a toggle on, so the app
//! registers itself the first time it runs from a bundle and leaves a marker
//! so it never does so again: switching it off in System Settings > General >
//! Login Items then sticks. `SMAppService` registers the running bundle's
//! path, which is why `make install` copies the app to /Applications first.

use objc2_foundation::NSBundle;
use objc2_service_management::{SMAppService, SMAppServiceStatus};

fn marker() -> Option<std::path::PathBuf> {
    Some(
        dirs::data_dir()?
            .join("wallbar")
            .join("login-item-registered"),
    )
}

fn has_bundle() -> bool {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .is_some_and(|id| !id.to_string().is_empty())
}

/// Register as a login item once. Returns what happened, for the log.
pub fn register_once() -> &'static str {
    if !has_bundle() {
        return "not running from a bundle; login item skipped";
    }
    let Some(marker) = marker() else {
        return "no Application Support folder; login item skipped";
    };
    if marker.exists() {
        return "login item already handled";
    }
    let service = unsafe { SMAppService::mainAppService() };
    let result = if unsafe { service.status() } == SMAppServiceStatus::Enabled {
        Ok(())
    } else {
        unsafe { service.registerAndReturnError() }
            .map_err(|e| e.localizedDescription().to_string())
    };
    match result {
        Ok(()) => {
            if let Some(dir) = marker.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&marker, b"");
            "registered as a login item"
        }
        Err(err) => {
            eprintln!("wallbar: could not register the login item: {err}");
            "login item registration failed"
        }
    }
}
