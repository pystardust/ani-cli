// Pre-build step for `package:win`: download the POSIX-side ani-cli
// dependencies that Git for Windows doesn't bundle, and stage them
// under `build-resources/win/bin/`. electron-builder copies the dir
// into the NSIS payload via `win.extraResources` in `package.json`,
// landing at `<install>/resources/bin/<binary>` at runtime. The Rust
// backend prepends that dir to the spawned bash's PATH (see
// `gui/backend/src/anicli/env.rs`) so ani-cli's `dep_ch` finds the
// bundled binaries before any system install.
//
// Why this exists: ani-cli's dep_ch surfaces missing deps via `die`,
// which `exit 1`s the entire shell — the `|| true` next to dep_ch
// calls only catches non-fatal returns, not explicit exits. So a
// Windows machine without these deps bricks playback / downloads
// silently with a generic "couldn't start this episode" or
// "download failed instantly" error. Bundling removes the
// dependency on the user's environment.
//
// Bundled today:
//   - fzf       — required for any spawn (dep_ch fzf at script start)
//   - aria2c    — required for downloads (dep_ch ffmpeg aria2c)
// Not bundled: ffmpeg. Too large (~80 MB compressed) to ship in the
// installer. Fetched on-demand by the backend when the user first
// triggers a download — see `gui/backend/src/anicli/ffmpeg.rs`.

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

// Each dep declares: pinned version, GitHub-releases URL, archive
// name + SHA-256, binary name (what gets dropped in build-resources),
// and the path to the binary inside the archive (so we can extract
// just the file, not the whole archive). The SHA-256 is captured on
// first download by re-running this script after a version bump:
// failure prints actual vs expected so you can paste the new hash.
const DEPS = [
	{
		name: 'fzf',
		version: '0.62.0',
		archiveName: 'fzf-0.62.0-windows_amd64.zip',
		url: 'https://github.com/junegunn/fzf/releases/download/v0.62.0/fzf-0.62.0-windows_amd64.zip',
		sha256: 'dac80c9d652c34f0ccd5d7c1c7b0e3ac9aa2e26c86d0a98b206ce5126f8a9774',
		binary: 'fzf.exe',
		// The fzf zip is flat — fzf.exe sits at the archive root.
		archivePath: 'fzf.exe',
	},
	{
		name: 'aria2',
		version: '1.37.0',
		archiveName: 'aria2-1.37.0-win-64bit-build1.zip',
		url: 'https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip',
		// Captured on first run — see SHA-256 mismatch error message
		// for how to refresh after a version bump.
		sha256: '67d015301eef0b612191212d564c5bb0a14b5b9c4796b76454276a4d28d9b288',
		binary: 'aria2c.exe',
		// aria2's zip nests the binary under a versioned directory.
		archivePath: 'aria2-1.37.0-win-64bit-build1/aria2c.exe',
	},
];

const cacheDir = path.join(electronDir, '.win-deps-cache');
const stagedBinDir = path.join(electronDir, 'build-resources', 'win', 'bin');

// Dev-mode parity: the Rust backend's `AppState::build` looks for
// bundled deps under `<resource_dir>/bin`, where `resource_dir` is
// the directory holding the backend exe. In dev that's
// `gui/backend/target/<profile>/`, so dropping the deps there makes
// the dev loop work without polluting global PATH or the system
// winget store. Both profiles get a copy so the user can switch
// between `cargo build` and `cargo build --release`.
const devTargetBinDirs = [
	path.join(repoRoot, 'gui', 'backend', 'target', 'debug', 'bin'),
	path.join(repoRoot, 'gui', 'backend', 'target', 'release', 'bin'),
];

async function sha256(filePath) {
	const buf = await readFile(filePath);
	return createHash('sha256').update(buf).digest('hex');
}

/**
 * Download `dep` into the local cache, verify the SHA-256, and return
 * the cached zip path. Cache hits are reused; mismatches trigger a
 * redownload. After a version bump, the placeholder SHA fails on
 * purpose and prints the real hash to copy back into this file.
 */
async function downloadOnce(dep) {
	const cachedZip = path.join(cacheDir, dep.archiveName);
	if (existsSync(cachedZip) && statSync(cachedZip).size > 0) {
		const got = await sha256(cachedZip);
		if (got === dep.sha256) {
			console.log(`[fetch-win-deps] cache hit: ${cachedZip}`);
			return cachedZip;
		}
		console.warn(`[fetch-win-deps] cached ${dep.name} checksum mismatch — redownloading`);
		await rm(cachedZip);
	}
	mkdirSync(cacheDir, { recursive: true });
	console.log(`[fetch-win-deps] downloading: ${dep.url}`);
	const resp = await fetch(dep.url, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status} for ${dep.url}`);
	await pipeline(resp.body, createWriteStream(cachedZip));
	const got = await sha256(cachedZip);
	if (got !== dep.sha256) {
		// Don't delete the cached file on first-run capture — keep it
		// so a re-run with the corrected hash hits the cache.
		throw new Error(
			`SHA-256 mismatch for ${dep.archiveName}\n` +
				`  expected: ${dep.sha256}\n` +
				`  got:      ${got}\n` +
				`If upstream rotated the asset (or this is a first-run version bump),` +
				` recompute and update DEPS[${dep.name}].sha256.`
		);
	}
	console.log(`[fetch-win-deps] verified ${dep.archiveName}`);
	return cachedZip;
}

/**
 * Use the system `tar` to extract a single named entry from a zip
 * into `destDir`. The entry path (`archivePath`) is preserved
 * relative to `destDir`, so deeply-nested binaries land in nested
 * subdirs that the caller flattens via `flattenInto`.
 *
 * Windows 10+ ships bsdtar (zip support included). Linux build hosts
 * need bsdtar via libarchive-tools — `tar` (GNU) on its own can't
 * read zips.
 */
function extractZipEntry(zipPath, archivePath, destDir) {
	return new Promise((resolve, reject) => {
		const proc = spawn('tar', ['-xf', zipPath, '-C', destDir, archivePath], {
			stdio: 'inherit',
			windowsHide: true,
		});
		proc.on('error', reject);
		proc.on('exit', (code) => {
			if (code === 0) resolve();
			else
				reject(
					new Error(
						`tar -xf '${archivePath}' from '${zipPath}' exited ${code}. ` +
							`On Linux build hosts, install bsdtar (apt: libarchive-tools).`
					)
				);
		});
	});
}

/**
 * Stage one dep: download the archive, extract just the binary, and
 * flatten it into the bin dir. Then mirror into the dev-mode cargo
 * target dirs that exist on disk.
 */
async function stageDep(dep) {
	const zip = await downloadOnce(dep);

	await mkdir(stagedBinDir, { recursive: true });
	const stagedBinary = path.join(stagedBinDir, dep.binary);
	if (existsSync(stagedBinary)) await rm(stagedBinary);

	// Extract the archive entry into a per-dep scratch dir so an
	// archive's nested layout doesn't collide with another dep's. Then
	// copy the binary into the flat staged bin dir.
	const scratchDir = path.join(cacheDir, `extract-${dep.name}`);
	await mkdir(scratchDir, { recursive: true });
	console.log(`[fetch-win-deps] extracting ${dep.binary} from ${dep.archiveName}`);
	await extractZipEntry(zip, dep.archivePath, scratchDir);

	const extractedBinary = path.join(scratchDir, dep.archivePath);
	if (!existsSync(extractedBinary)) {
		throw new Error(`expected ${extractedBinary} after extracting ${dep.archiveName}`);
	}
	await copyFile(extractedBinary, stagedBinary);
	console.log(`[fetch-win-deps] staged → ${stagedBinary}`);

	for (const devDir of devTargetBinDirs) {
		// Only populate if the cargo target exists — don't create it
		// from scratch (cargo would later wipe it). Skipping when the
		// target dir is absent keeps the script no-op on machines that
		// haven't run `cargo build` yet.
		const profileDir = path.dirname(devDir);
		if (!existsSync(profileDir)) continue;
		await mkdir(devDir, { recursive: true });
		const devBinary = path.join(devDir, dep.binary);
		if (existsSync(devBinary)) await rm(devBinary);
		await copyFile(stagedBinary, devBinary);
		console.log(`[fetch-win-deps] dev copy → ${devBinary}`);
	}
}

async function main() {
	for (const dep of DEPS) {
		console.log(`[fetch-win-deps] === ${dep.name} ${dep.version} ===`);
		await stageDep(dep);
	}
	console.log(
		`[fetch-win-deps] done — ${DEPS.map((d) => `${d.name} ${d.version}`).join(', ')} staged for Windows packaging`
	);
}

main().catch((err) => {
	console.error('[fetch-win-deps] failed:', err.message);
	process.exit(1);
});
