mod auth;
mod commands;
mod errors;
mod github;
mod legacy;
mod models;
mod oauth;
mod secure_store;
use tauri::Manager;
#[cfg(test)]
mod tests;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(
                    "sqlite:stickynote.db",
                    vec![
                        tauri_plugin_sql::Migration {
                            version: 1,
                            description: "initial schema",
                            sql: include_str!("../migrations/001_initial.sql"),
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 2,
                            description: "note tags",
                            sql: include_str!("../migrations/002_note_tags.sql"),
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("secure-auth")
                .setup(|_app, _api| {
                    #[cfg(target_os = "android")]
                    _app.manage(secure_store::AndroidBridge(_api.register_android_plugin(
                        "com.gitsticky.stickynote",
                        "SecureAuthPlugin",
                    )?));
                    Ok(())
                })
                .build(),
        )
        .setup(|app| {
            app.manage(auth::Credentials::new(
                auth::AppConfig::bundled(),
                std::sync::Arc::new(secure_store::NativeStore(app.handle().clone())),
                std::sync::Arc::new(oauth::GitHubOAuth::new().map_err(|_| {
                    std::io::Error::other("Could not initialize GitHub authentication")
                })?),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::db_snapshot,
            commands::db_save,
            commands::db_settings,
            commands::db_apply,
            commands::db_ack,
            auth::auth_status,
            auth::auth_begin,
            auth::auth_poll,
            auth::auth_cancel,
            auth::auth_disconnect,
            auth::auth_open_browser,
            auth::auth_import_legacy,
            github::github_repositories,
            github::github_branches,
            github::github_test,
            github::github_list,
            github::github_read,
            github::github_write,
            github::github_remove
        ])
        .run(tauri::generate_context!())
        .expect("StickyNote could not start");
}
