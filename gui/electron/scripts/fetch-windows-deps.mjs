// Pre-build step for `package:win`: download the POSIX-side ani-cli
// dependencies that Git for Windows doesn't bundle (today: fzf), and
// stage them under `build-resources/win/bin/`. electron-builder copies
// the dir into the NSIS payload via `win.extraResources` in
// `package.json`, landing at `<install>/resources/bin/fzf.exe` at
// runtime. The Rust backend prepends that dir to the spawned bash's
// PATH (see `gui/backend/src/anicli/env.rs`) so ani-cli's `dep_ch`
// finds the bundled binary before any system install.
//
// Why this exists: ani-cli's `dep_ch fzf || true` doesn't actually
// catch a missing fzf, because `dep_ch` calls `die` which `exit 1`s
// the entire shell — the `|| true` only fires for a non-fatal
// non-zero return. So a Windows machine without fzf bricks playback
// silently with a generic "couldn't start this episode" error.
// Bundling fzf removes the dependency on the user's environment.

import { createWriteStream, existsSync, mkdirSync, statSync } from 'node:fs';
import { copyFile, mkdir, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const electronDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(electronDir, '..', '..');

// Pinned fzf version + checksum for reproducibility. Update both when
// bumping. Checksum from the upstream release page; verified by the
// script before staging the binary.
const FZF_VERSION = '0.62.0';
const FZF_ZIP_NAME = `fzf-${FZF_VERSION}-windows_amd64.zip`;
const FZF_URL = `https://github.com/junegunn/fzf/releases/download/v${FZF_VERSION}/${FZF_ZIP_NAME}`;
// SHA-256 of the windows_amd64 zip from upstream's release page.
// If this script fails verification after a version bump, recompute
// with: `Get-FileHash -Algorithm SHA256 fzf-X.Y.Z-windows_amd64.zip`
// or `sha256sum fzf-X.Y.Z-windows_amd64.zip` and update both fields.
const FZF_ZIP_SHA256 = 'dac80c9d652c34f0ccd5d7c1c7b0e3ac9aa2e26c86d0a98b206ce5126f8a9774';

const cacheDir = path.join(electronDir, '.win-deps-cache');
const stagedBinDir = path.join(electronDir, 'build-resources', 'win', 'bin');

// Dev-mode parity: the Rust backend's `AppState::build` looks for
// bundled deps under `<resource_dir>/bin`, where `resource_dir` is
// the directory holding the backend exe. In dev that's
// `gui/backend/target/<profile>/`, so dropping fzf there makes the
// dev loop work without polluting global PATH or the system winget
// store. Both profiles get a copy so the user can switch between
// `cargo build` and `cargo build --release`.
const devTargetBinDirs = [
	path.join(repoRoot, 'gui', 'backend', 'target', 'debug', 'bin'),
	path.join(repoRoot, 'gui', 'backend', 'target', 'release', 'bin'),
];

async function sha256(filePath) {
	const buf = await readFile(filePath);
	return createHash('sha256').update(buf).digest('hex');
}

async function downloadOnce() {
	const cachedZip = path.join(cacheDir, FZF_ZIP_NAME);
	if (existsSync(cachedZip) && statSync(cachedZip).size > 0) {
		const got = await sha256(cachedZip);
		if (got === FZF_ZIP_SHA256) {
			console.log(`[fetch-win-deps] cache hit: ${cachedZip}`);
			return cachedZip;
		}
		console.warn(`[fetch-win-deps] cached zip checksum mismatch — redownloading`);
		await rm(cachedZip);
	}
	mkdirSync(cacheDir, { recursive: true });
	console.log(`[fetch-win-deps] downloading: ${FZF_URL}`);
	const resp = await fetch(FZF_URL, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status}`);
	await pipeline(resp.body, createWriteStream(cachedZip));
	const got = await sha256(cachedZip);
	if (got !== FZF_ZIP_SHA256) {
		await rm(cachedZip);
		throw new Error(
			`SHA-256 mismatch for ${FZF_ZIP_NAME}\n` +
				`  expected: ${FZF_ZIP_SHA256}\n` +
				`  got:      ${got}\n` +
				`If upstream rotated the asset, recompute and update FZF_ZIP_SHA256.`
		);
	}
	console.log(`[fetch-win-deps] verified ${FZF_ZIP_NAME}`);
	return cachedZip;
}

// Use the system `tar` to extract — Windows 10+ ships bsdtar (zip
// support included), and Linux build hosts can install bsdtar via
// `libarchive-tools`. Avoids adding adm-zip / yauzl as a runtime
// devDep just to crack open one file.
function extractFzfExe(zipPath, destDir) {
	return new Promise((resolve, reject) => {
		const proc = spawn('tar', ['-xf', zipPath, '-C', destDir, 'fzf.exe'], {
			stdio: 'inherit',
			windowsHide: true,
		});
		proc.on('error', reject);
		proc.on('exit', (code) => {
			if (code === 0) resolve();
			else
				reject(
					new Error(
						`tar -xf exited ${code}. ` +
							`On Linux build hosts, install bsdtar (apt: libarchive-tools).`
					)
				);
		});
	});
}

async function main() {
	const zip = await downloadOnce();

	await mkdir(stagedBinDir, { recursive: true });
	const stagedExe = path.join(stagedBinDir, 'fzf.exe');
	if (existsSync(stagedExe)) await rm(stagedExe);
	console.log(`[fetch-win-deps] extracting fzf.exe → ${stagedExe}`);
	await extractFzfExe(zip, stagedBinDir);

	for (const devDir of devTargetBinDirs) {
		// Only populate if the cargo target exists — don't create it
		// from scratch (cargo would later wipe it). Skipping when the
		// target dir is absent keeps the script no-op on machines that
		// haven't run `cargo build` yet.
		const profileDir = path.dirname(devDir);
		if (!existsSync(profileDir)) continue;
		await mkdir(devDir, { recursive: true });
		const devExe = path.join(devDir, 'fzf.exe');
		if (existsSync(devExe)) await rm(devExe);
		await copyFile(stagedExe, devExe);
		console.log(`[fetch-win-deps] dev copy → ${devExe}`);
	}
	console.log(`[fetch-win-deps] done — fzf ${FZF_VERSION} staged for Windows packaging`);
}

main().catch((err) => {
	console.error('[fetch-win-deps] failed:', err.message);
	process.exit(1);
});
