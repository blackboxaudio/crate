fn main() {
    bake_cloud_sync_config();
    tauri_build::build()
}

/// Bake the cloud-sync config into the binary so a mobile build can find it at runtime.
///
/// On a device the app runs sandboxed and can't read `cloud_sync.config.json` from the repo, so it
/// relies on the compile-time `option_env!("GCLOUD_*")` fallback
/// (`services/cloud_sync/config.rs::from_compiled_env`). Feeding those through a shell env var is
/// unreliable on mobile: the Rust build runs inside an Xcode "Run Script" phase (and the Android
/// Gradle task), which do not pass arbitrary inherited environment through to `cargo`/`rustc` — and
/// building via Xcode's Run button bypasses the dev script entirely.
///
/// So for mobile targets we read `cloud_sync.config.json` here — it is on disk in the package root
/// at build time — and emit `cargo:rustc-env=GCLOUD_*=<value>`, the canonical way to set a variable
/// that `option_env!` resolves. An already-set `GCLOUD_*` env var (release/CI secrets) takes
/// precedence. A missing file or field is fine: the value stays unset and cloud sync is simply
/// unavailable on the device, never a build failure.
fn bake_cloud_sync_config() {
    // (cloud_sync.config.json field, compile-time env var read by config.rs::from_compiled_env)
    const FIELDS: &[(&str, &str)] = &[
        ("project_id", "GCLOUD_PROJECT_ID"),
        ("web_api_key", "GCLOUD_WEB_API_KEY"),
        ("storage_bucket", "GCLOUD_STORAGE_BUCKET"),
        ("oauth_client_id", "GCLOUD_OAUTH_CLIENT_ID"),
        ("oauth_client_secret", "GCLOUD_OAUTH_CLIENT_SECRET"),
        ("ios_oauth_client_id", "GCLOUD_IOS_OAUTH_CLIENT_ID"),
        ("android_oauth_client_id", "GCLOUD_ANDROID_OAUTH_CLIENT_ID"),
        ("firebase_ios_app_id", "GCLOUD_FIREBASE_IOS_APP_ID"),
        ("firebase_android_app_id", "GCLOUD_FIREBASE_ANDROID_APP_ID"),
        ("appcheck_debug_token", "GCLOUD_APPCHECK_DEBUG_TOKEN"),
    ];

    println!("cargo:rerun-if-changed=cloud_sync.config.json");
    for &(_, var) in FIELDS {
        println!("cargo:rerun-if-env-changed={var}");
    }

    // Desktop reads the file at runtime (dev) or ambient GCLOUD_* (release/CI); only mobile device
    // builds, which can't read the file from the sandbox, need it baked in here.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "ios" && target_os != "android" {
        return;
    }

    let config: Option<serde_json::Value> = std::fs::read_to_string("cloud_sync.config.json")
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok());

    for &(field, var) in FIELDS {
        // An explicitly-set env var (CI secrets) takes precedence; option_env! reads it directly.
        if std::env::var_os(var)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        {
            continue;
        }
        let value = config
            .as_ref()
            .and_then(|c| c.get(field))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(value) = value {
            println!("cargo:rustc-env={var}={value}");
        }
    }
}
