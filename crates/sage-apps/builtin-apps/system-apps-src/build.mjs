import { execFileSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, 'apps');

const apps = readdirSync(root)
  .map((name) => ({ name, dir: join(root, name) }))
  .filter((entry) => statSync(entry.dir).isDirectory());

for (const app of apps) {
  console.log(`\n==> Building system app: ${app.name}`);
  execFileSync(
    process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm',
    ['exec', 'vite', 'build', '--config', join(app.dir, 'vite.config.ts')],
    {
      stdio: 'inherit',
      cwd: resolve(import.meta.dirname),
    },
  );
}
