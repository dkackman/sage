use std::path::Path;

use super::finalize::finalize_prebuilt_app;

#[derive(Clone, Copy)]
struct SystemApp {
    app_dir_name: &'static str,
    out_dir_name: &'static str,
}

const SYSTEM_BUILD_PLAN: &[SystemApp] = &[SystemApp {
    app_dir_name: "task-manager",
    out_dir_name: "task-manager",
}];

pub fn build_system_apps(
    system_apps_src_dir: &Path,
    system_out_dir: &Path,
    system_sdk_dist: &Path,
) -> Result<(), String> {
    for system_app in SYSTEM_BUILD_PLAN {
        let app_src_dir = system_apps_src_dir.join(system_app.app_dir_name);
        let built_dir = system_out_dir.join(system_app.out_dir_name);
        let manifest_src = app_src_dir.join("sage-manifest.json");

        finalize_prebuilt_app(
            &built_dir,
            &manifest_src,
            &system_out_dir.join(system_app.out_dir_name),
            system_sdk_dist,
        )?;
    }

    Ok(())
}
