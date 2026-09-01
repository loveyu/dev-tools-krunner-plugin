import { spawn, type ChildProcess } from 'node:child_process';
import { readFileSync, rmSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import { createServer, createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const host = '127.0.0.1';
const firstPort = 7173;
const frontendOnly = process.argv.includes('--frontend-only');
const portFile = join(tmpdir(), 'devtools-workerd-vite.port');
const port = await findAvailablePort();
const url = `http://${host}:${String(port)}`;
const frontendRoot = resolve(import.meta.dirname, '..');
const workspaceRoot = resolve(frontendRoot, '../..');
const viteEntry = resolve(frontendRoot, 'node_modules/vite/bin/vite.js');
const vite = spawn(
  process.execPath,
  [viteEntry, '--host', host, '--port', String(port), '--strictPort'],
  { cwd: frontendRoot, stdio: 'inherit' },
);
const viteExit = waitForExit(vite);
let worker: ChildProcess | null = null;

const stopChildren = (): void => {
  worker?.kill();
  vite.kill();
};
process.once('SIGINT', () => {
  stopChildren();
});
process.once('SIGTERM', () => {
  stopChildren();
});
process.once('exit', removeOwnPortFile);

try {
  await Promise.race([
    waitForServer(port),
    viteExit.then((code) => {
      throw new Error(`Vite 在页面可用前退出，退出码 ${String(code)}`);
    }),
  ]);
  await writeFile(portFile, `${String(port)}\n`, { encoding: 'utf8', mode: 0o600 });
  console.log(`WebView 调试地址：${url}`);
  console.log(`端口文件：${portFile}`);

  if (frontendOnly) {
    process.exitCode = (await viteExit) ?? 1;
  } else {
    worker = spawn(cargoCommand(), ['run', '-p', 'devtools-workerd', '--', '--launcher'], {
      cwd: workspaceRoot,
      env: { ...process.env, DEVTOOLS_WEBVIEW_DEBUG: '1' },
      stdio: 'inherit',
    });
    const result = await Promise.race([
      waitForExit(worker).then((code) => ({ code, source: 'worker' as const })),
      viteExit.then((code) => ({ code, source: 'vite' as const })),
    ]);
    if (result.source === 'vite') worker.kill();
    process.exitCode = result.code ?? 1;
  }
} finally {
  stopChildren();
  removeOwnPortFile();
}

async function findAvailablePort(): Promise<number> {
  for (let candidate = firstPort; candidate <= 65_535; candidate += 1) {
    if (await canListen(candidate)) return candidate;
  }
  throw new Error(`从 ${String(firstPort)} 起没有可用的 TCP 端口`);
}

function canListen(candidate: number): Promise<boolean> {
  return new Promise((resolveListen, reject) => {
    const server = createServer();
    server.once('error', (error: NodeJS.ErrnoException) => {
      if (error.code === 'EADDRINUSE' || error.code === 'EACCES') {
        resolveListen(false);
      } else {
        reject(error);
      }
    });
    server.listen(candidate, host, () => {
      server.close((error) => {
        if (error === undefined) resolveListen(true);
        else reject(error);
      });
    });
  });
}

async function waitForServer(serverPort: number): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await canConnect(serverPort)) return;
    await delay(50);
  }
  throw new Error(`Vite 启动超时：${url}`);
}

function canConnect(serverPort: number): Promise<boolean> {
  return new Promise((resolveConnection) => {
    const socket = createConnection({ host, port: serverPort });
    socket.once('connect', () => {
      socket.end();
      resolveConnection(true);
    });
    socket.once('error', () => {
      resolveConnection(false);
    });
  });
}

function waitForExit(child: ChildProcess): Promise<number | null> {
  return new Promise((resolveExit, reject) => {
    child.once('error', reject);
    child.once('exit', (code) => {
      resolveExit(code);
    });
  });
}

function removeOwnPortFile(): void {
  try {
    if (readFileSync(portFile, 'utf8').trim() === String(port)) {
      rmSync(portFile, { force: true });
    }
  } catch {
    // 文件可能未创建，或已由另一个调试会话替换。
  }
}

function cargoCommand(): string {
  return process.platform === 'win32' ? 'cargo.exe' : 'cargo';
}
