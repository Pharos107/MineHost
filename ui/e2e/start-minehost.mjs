// Chạy binary `minehost` thật ở chế độ e2e cho Playwright: thư mục dữ liệu mới mỗi lần, Java giả
// (`fake-mc`), cổng riêng để không đụng MineHost đang chạy trên máy. Xem crates/minehost/README.md.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const exe = process.platform === "win32" ? ".exe" : "";
const minehost = join(repo, "target/debug", `minehost${exe}`);
const fakeJava = join(repo, "target/debug", `fake-mc${exe}`);
const port = process.env.MINEHOST_E2E_PORT ?? "7879";

for (const bin of [minehost, fakeJava]) {
  if (!existsSync(bin)) {
    console.error(`Thiếu ${bin}. Chạy: cargo build -p mh-runtime --bin fake-mc && cargo build -p minehost --features e2e`);
    process.exit(1);
  }
}

const dataDir = join(tmpdir(), `minehost-e2e-${port}`);
rmSync(dataDir, { recursive: true, force: true });
mkdirSync(dataDir, { recursive: true });
writeFileSync(join(dataDir, "config.toml"), `listen = "127.0.0.1:${port}"\nopen_browser = false\nlog_level = "info"\n`);

const child = spawn(minehost, ["--data-dir", dataDir, "--no-browser", "--no-tray"], {
  env: { ...process.env, MINEHOST_E2E_JAVA: fakeJava },
  stdio: "inherit",
});
child.on("exit", (code) => process.exit(code ?? 0));
// Playwright dừng script này khi xong; dừng luôn controller để không còn process sót.
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));
