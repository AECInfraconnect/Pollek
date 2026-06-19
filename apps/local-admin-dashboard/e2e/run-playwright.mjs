import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const externalServer = process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === '1';
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? (
  externalServer ? 'http://127.0.0.1:3000' : 'http://127.0.0.1:5173'
);

let vite;

async function waitForServer(url) {
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Server is still starting.
    }
    await delay(500);
  }
  throw new Error(`Timed out waiting for ${url}`);
}

function stopVite() {
  if (!vite?.pid) {
    return;
  }
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/pid', String(vite.pid), '/t', '/f'], { stdio: 'ignore' });
  } else {
    vite.kill('SIGTERM');
  }
}

function runNode(args, env) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, args, {
      stdio: 'inherit',
      env,
    });
    child.on('exit', (code, signal) => {
      resolve(code ?? (signal ? 1 : 0));
    });
  });
}

process.on('exit', stopVite);
process.on('SIGINT', () => {
  stopVite();
  process.exit(130);
});
process.on('SIGTERM', () => {
  stopVite();
  process.exit(143);
});

if (!externalServer) {
  const viteCli = path.resolve('node_modules', 'vite', 'bin', 'vite.js');
  vite = spawn(process.execPath, [viteCli, '--host', '127.0.0.1', '--port', '5173'], {
    stdio: 'inherit',
    env: process.env,
  });
  await waitForServer(baseURL);
}

const playwrightCli = path.resolve('node_modules', '@playwright', 'test', 'cli.js');
const exitCode = await runNode([playwrightCli, 'test', ...process.argv.slice(2)], {
  ...process.env,
  PLAYWRIGHT_BASE_URL: baseURL,
});

stopVite();
process.exit(exitCode);
