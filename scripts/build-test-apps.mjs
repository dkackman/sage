import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const BUILTIN_ROOT = path.join(ROOT, 'src/builtin-apps');
const SHARED_DIR = path.join(BUILTIN_ROOT, 'shared');
const TEST_SRC_DIR = path.join(BUILTIN_ROOT, 'test-apps-src');
const RUNTIME_SRC_DIR = path.join(BUILTIN_ROOT, 'runtime-apps-src');
const DIST_ROOT = path.join(BUILTIN_ROOT, 'dist');
const TEST_OUT_DIR = path.join(DIST_ROOT, 'test-apps');
const RUNTIME_OUT_DIR = path.join(DIST_ROOT, 'runtime-apps');
const SDK_DIST = path.join(ROOT, 'packages/sage-app-sdk/dist');

async function rmrf(dir) {
  await fs.rm(dir, { recursive: true, force: true });
}

async function mkdirp(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function exists(p) {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

async function copyDir(src, dst) {
  if (!(await exists(src))) {
    throw new Error(`Missing directory: ${src}`);
  }

  await mkdirp(dst);

  const entries = await fs.readdir(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const dstPath = path.join(dst, entry.name);

    if (entry.isDirectory()) {
      await copyDir(srcPath, dstPath);
    } else if (entry.isFile()) {
      await mkdirp(path.dirname(dstPath));
      await fs.copyFile(srcPath, dstPath);
    }
  }
}

async function copyFileRequired(src, dst, label) {
  if (!(await exists(src))) {
    throw new Error(`Missing ${label} at ${src}`);
  }

  await mkdirp(path.dirname(dst));
  await fs.copyFile(src, dst);
}

async function finalizeBuiltApp({
  sourceDir,
  outDir,
  manifestFileName = null,
}) {
  await mkdirp(outDir);

  if (await exists(SHARED_DIR)) {
    await copyDir(SHARED_DIR, outDir);
  }

  await copyDir(sourceDir, outDir);

  const files = await fs.readdir(outDir);

  for (const file of files) {
    if (file.startsWith('sage-manifest') && file.endsWith('.json')) {
      await fs.rm(path.join(outDir, file), { force: true });
    }
  }

  if (manifestFileName) {
    await copyFileRequired(
      path.join(sourceDir, manifestFileName),
      path.join(outDir, 'sage-manifest.json'),
      `manifest ${manifestFileName}`,
    );
  }

  await copyFileRequired(
    path.join(SDK_DIST, 'runtime-bridge.js'),
    path.join(outDir, 'bridge.js'),
    'SDK runtime bridge build output',
  );

  await copyFileRequired(
    path.join(SDK_DIST, 'index.js'),
    path.join(outDir, 'sdk.js'),
    'SDK index build output',
  );
}

const TEST_BUILD_PLAN = [
  {
    sourceDirName: 'sage-storage-isolation',
    variants: [
      {
        outDirName: 'sage-storage-isolation-persistent',
        manifestFileName: 'sage-manifest.persistent.json',
      },
      {
        outDirName: 'sage-storage-isolation-incognito',
        manifestFileName: 'sage-manifest.incognito.json',
      },
    ],
  },
  {
    sourceDirName: 'storage-persistence',
    variants: [
      {
        outDirName: 'storage-persistence-persistent',
        manifestFileName: 'sage-manifest.persistent.json',
      },
      {
        outDirName: 'storage-persistence-incognito',
        manifestFileName: 'sage-manifest.incognito.json',
      },
    ],
  },
  {
    sourceDirName: 'network-allow-a',
    variants: [
      {
        outDirName: 'network-allow-a',
        manifestFileName: 'sage-manifest.json',
      },
    ],
  },
  {
    sourceDirName: 'network-allow-b',
    variants: [
      {
        outDirName: 'network-allow-b',
        manifestFileName: 'sage-manifest.json',
      },
    ],
  },
];

const RUNTIME_BUILD_PLAN = [
  {
    sourceDirName: 'storage-clear-probe',
    outDirName: 'storage-clear-probe',
  },
];

async function main() {
  console.log('→ building builtin apps');

  await rmrf(DIST_ROOT);
  await mkdirp(TEST_OUT_DIR);
  await mkdirp(RUNTIME_OUT_DIR);

  for (const group of TEST_BUILD_PLAN) {
    for (const variant of group.variants) {
      console.log(`  - test ${variant.outDirName}`);
      await finalizeBuiltApp({
        sourceDir: path.join(TEST_SRC_DIR, group.sourceDirName),
        outDir: path.join(TEST_OUT_DIR, variant.outDirName),
        manifestFileName: variant.manifestFileName,
      });
    }
  }

  for (const runtimeApp of RUNTIME_BUILD_PLAN) {
    console.log(`  - runtime ${runtimeApp.outDirName}`);
    await finalizeBuiltApp({
      sourceDir: path.join(RUNTIME_SRC_DIR, runtimeApp.sourceDirName),
      outDir: path.join(RUNTIME_OUT_DIR, runtimeApp.outDirName),
      manifestFileName: null,
    });
  }

  console.log('✓ builtin apps ready');
}

main().catch((err) => {
  console.error('build-test-apps failed:', err);
  process.exit(1);
});
