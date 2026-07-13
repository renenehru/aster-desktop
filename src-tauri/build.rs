const COMMANDS: &[&str] = &[
    "app_status",
    "credential_status",
    "acknowledge_external_processing",
    "prompt_store_api_key",
    "delete_api_key",
    "list_conversations",
    "get_conversation",
    "create_conversation",
    "rename_conversation",
    "delete_conversation",
    "send_message",
    "cancel_generation",
    "export_conversation",
    "import_conversations",
    "open_external_url",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("Tauri build configuration is invalid");
}
