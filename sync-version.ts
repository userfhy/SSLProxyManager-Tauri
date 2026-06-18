type FsModule = {
  readFileSync(filePath: string, encoding: "utf8"): string;
  writeFileSync(filePath: string, data: string, encoding: "utf8"): void;
};

type PathModule = {
  join(...paths: string[]): string;
};

declare const require: {
  (moduleName: "node:fs"): FsModule;
  (moduleName: "node:path"): PathModule;
};
declare const process: {
  cwd(): string;
};

const fs = require("node:fs");
const path = require("node:path");

const root = process.cwd();

function readCargoVersion(cargoTomlPath: string): string {
  const txt = fs.readFileSync(cargoTomlPath, "utf8");
  const m = txt.match(/^version\s*=\s*"([^"]+)"/m);
  if (!m) {
    throw new Error(`无法从 ${cargoTomlPath} 解析 version`);
  }
  return m[1];
}

function syncJsonFile(
  filePath: string,
  version: string,
): { changed: boolean; old: string | undefined } {
  const raw = fs.readFileSync(filePath, "utf8");
  const json = JSON.parse(raw) as { version?: string };
  const old = json.version;

  if (old === version) {
    return { changed: false, old };
  }

  json.version = version;
  fs.writeFileSync(filePath, JSON.stringify(json, null, 2) + "\n", "utf8");
  return { changed: true, old };
}

const cargoToml = path.join(root, "Cargo.toml");
const tauriConf = path.join(root, "tauri.conf.json");
const packageJson = path.join(root, "package.json");

const version = readCargoVersion(cargoToml);

const rTauri = syncJsonFile(tauriConf, version);
if (rTauri.changed) {
  console.log(`tauri.conf.json version: ${rTauri.old} -> ${version}`);
} else {
  console.log(`tauri.conf.json version 已是 ${version}`);
}

const rPkg = syncJsonFile(packageJson, version);
if (rPkg.changed) {
  console.log(`package.json version: ${rPkg.old} -> ${version}`);
} else {
  console.log(`package.json version 已是 ${version}`);
}
