use std::path::Path;

use super::finalize::finalize_source_app;

#[derive(Clone, Copy)]
struct RuntimeApp {
    source_dir_name: &'static str,
    out_dir_name: &'static str,
}

const RUNTIME_BUILD_PLAN: &[RuntimeApp] = &[RuntimeApp {
    source_dir_name: "storage-clear-probe",
    out_dir_name: "storage-clear-probe",
}];

pub fn build_runtime_apps(
    runtime_src_dir: &Path,
    runtime_out_dir: &Path,
    user_sdk_dist: &Path,
) -> Result<(), String> {
    for runtime_app in RUNTIME_BUILD_PLAN {
        finalize_source_app(
            None,
            &runtime_src_dir.join(runtime_app.source_dir_name),
            &runtime_out_dir.join(runtime_app.out_dir_name),
            None,
            user_sdk_dist,
        )?;
    }

    Ok(())
}
