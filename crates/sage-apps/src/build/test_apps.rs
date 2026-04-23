use std::path::Path;

use super::finalize::finalize_source_app;

#[derive(Clone, Copy)]
struct TestVariant {
    out_dir_name: &'static str,
    manifest_file_name: &'static str,
}

#[derive(Clone, Copy)]
struct TestGroup {
    source_dir_name: &'static str,
    variants: &'static [TestVariant],
}

const TEST_BUILD_PLAN: &[TestGroup] = &[
    TestGroup {
        source_dir_name: "sage-storage-isolation",
        variants: &[
            TestVariant {
                out_dir_name: "sage-storage-isolation-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
            TestVariant {
                out_dir_name: "sage-storage-isolation-incognito",
                manifest_file_name: "sage-manifest.incognito.json",
            },
        ],
    },
    TestGroup {
        source_dir_name: "storage-persistence",
        variants: &[
            TestVariant {
                out_dir_name: "storage-persistence-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
            TestVariant {
                out_dir_name: "storage-persistence-incognito",
                manifest_file_name: "sage-manifest.incognito.json",
            },
            TestVariant {
                out_dir_name: "storage-clear-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
        ],
    },
    TestGroup {
        source_dir_name: "network-allow-a",
        variants: &[TestVariant {
            out_dir_name: "network-allow-a",
            manifest_file_name: "sage-manifest.json",
        }],
    },
    TestGroup {
        source_dir_name: "network-allow-b",
        variants: &[TestVariant {
            out_dir_name: "network-allow-b",
            manifest_file_name: "sage-manifest.json",
        }],
    },
];

pub fn build_test_apps(
    test_src_dir: &Path,
    test_out_dir: &Path,
    user_sdk_dist: &Path,
) -> Result<(), String> {
    let shared_test_dir = test_src_dir.join("_shared");

    for group in TEST_BUILD_PLAN {
        for variant in group.variants {
            finalize_source_app(
                Some(&shared_test_dir),
                &test_src_dir.join(group.source_dir_name),
                &test_out_dir.join(variant.out_dir_name),
                Some(variant.manifest_file_name),
                user_sdk_dist,
            )?;
        }
    }

    Ok(())
}
