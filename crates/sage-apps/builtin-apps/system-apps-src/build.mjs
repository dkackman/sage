import { execFileSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const packageRoot = resolve(import.meta.dirname);
const appsRoot = resolve(import.meta.dirname, 'apps');

const pnpm = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';

const apps = readdirSync(appsRoot)
  .map((name) => ({ name, dir: join(appsRoot, name) }))
  .filter((entry) => statSync(entry.dir).isDirectory());

for (const app of apps) {
  console.log(`\n==> Typechecking system app: ${app.name}`);
  execFileSync(
    pnpm,
    ['exec', 'tsc', '--noEmit', '--project', join(app.dir, 'tsconfig.json')],
    {
      stdio: 'inherit',
      cwd: packageRoot,
    },
  );

  console.log(`\n==> Building system app: ${app.name}`);
  execFileSync(
    pnpm,
    ['exec', 'vite', 'build', '--config', join(app.dir, 'vite.config.ts')],
    {
      stdio: 'inherit',
      cwd: packageRoot,
    },
  );
}
