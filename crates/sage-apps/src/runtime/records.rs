use crate::runtime::state::types::SageAppRuntimeKind;

pub fn runtime_id_for(app_id: &str, runtime_kind: SageAppRuntimeKind) -> String {
    match runtime_kind {
        SageAppRuntimeKind::User => format!("runtime-{app_id}"),
        SageAppRuntimeKind::System => format!("system-runtime-{app_id}"),
    }
}

pub fn inline_label_for(app_id: &str, runtime_kind: SageAppRuntimeKind) -> String {
    match runtime_kind {
        SageAppRuntimeKind::User => format!("app-inline-{app_id}"),
        SageAppRuntimeKind::System => format!("system-app-inline-{app_id}"),
    }
}
