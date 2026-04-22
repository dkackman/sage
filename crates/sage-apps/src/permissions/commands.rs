use crate::types::{
    SageAppCapabilityDefinitionView, SageAppCapabilityFlagsView,
};

#[tauri::command]
#[specta::specta]
pub async fn apps_get_capability_registry(
) -> Result<Vec<SageAppCapabilityDefinitionView>, String> {
    let entries = crate::permissions::registry()
        .into_values()
        .map(|definition| SageAppCapabilityDefinitionView {
            key: definition.key.to_string(),
            label: definition.label.to_string(),
            description: definition.description.to_string(),
            flags: SageAppCapabilityFlagsView {
                externally_observable: definition.flags.externally_observable,
                accesses_sensitive_secret: definition.flags.accesses_sensitive_secret,
                persistent_storage: definition.flags.persistent_storage,
            },
            requestable_by_app: definition.requestable_by_app,
            shared_with_app: definition.shared_with_app,
        })
        .collect();

    Ok(entries)
}
