use std::collections::BTreeMap;
use tauri::{
    ipc::Origin,
    utils::{
        acl::{capability::Capability, manifest::Manifest, resolved::Resolved},
        platform::Target,
    },
};

#[test]
fn application_acl_only_allows_the_trusted_local_main_webview() {
    // Use production build output and the real resolver/authority without a GUI.
    let manifests: BTreeMap<String, Manifest> =
        serde_json::from_str(include_str!("../gen/schemas/acl-manifests.json")).unwrap();
    let capabilities: BTreeMap<String, Capability> =
        serde_json::from_str(include_str!("../gen/schemas/capabilities.json")).unwrap();
    let resolved = Resolved::resolve(&manifests, capabilities, Target::current()).unwrap();
    assert!(resolved.has_app_acl);
    let commands = resolved
        .allowed_commands
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(commands.contains(&"security_rotate_sync_key".into()));
    assert!(commands.contains(&"db_snapshot".into()));
    let authority = tauri::runtime_authority!(manifests, resolved);
    for command in commands {
        assert!(authority
            .resolve_access(&command, "main", "main", &Origin::Local)
            .is_some());
        assert!(authority
            .resolve_access(&command, "untrusted", "untrusted", &Origin::Local)
            .is_none());
        assert!(authority
            .resolve_access(
                &command,
                "main",
                "main",
                &Origin::Remote {
                    url: "https://example.com/".parse().unwrap()
                }
            )
            .is_none());
    }
    assert!(authority
        .resolve_access("plugin:window|create", "main", "main", &Origin::Local)
        .is_none());
}
