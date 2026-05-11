// Pre-build step for `package:release`: download the POSIX-side
// ani-cli dependencies that aren't safe to assume on every Linux
// host, and stage them under `build-resources/linux/bin/`. electron-
// builder copies the dir into both the .deb and AppImage payloads
// via `linux.extraResources` in `package.json`, landing at
// `<install>/resources/bin/<binary>` at runtime. The Rust backend
// prepends that dir to the spawned bash's PATH (see
// `gui/backend/src/anicli/env.rs`) so ani-cli's `dep_ch` finds the
// bundled binaries before any system install.
//
// Why this exists: ani-cli's dep_ch surfaces missing deps via `die`,
// which `exit 1`s the entire shell. A Linux desktop without these
// tools bricks playback or downloads silently. Bundling the small
// fast ones (fzf, aria2c) removes that footgun.
//
// Bundled today:
//   - fzf      — required for any spawn (dep_ch fzf at script start)
//   - aria2c   — required for downloads (dep_ch ffmpeg aria2c)
//
// NOT bundled, by design:
//   - ffmpeg   — too large (~80 MB compressed). Declared as a
//                `Recommends:` on the .deb so apt pulls the distro
//                build; AppImage users fall back to system PATH or
//                see the typed FfmpegMissing modal. Mirrors the
//                Windows installer which fetches ffmpeg at install
//                time rather than embedding it in the .exe.
//
// This script is the Linux analog of `fetch-windows-deps.mjs`. Keep
// the two in lockstep when adding a new bundled dep.

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

// Each dep declares: pinned version, download URL, archive name +
// SHA-256, binary name (what gets dropped in build-resources), and
// the path to the binary inside the archive (so we can extract just
// the file). SHA-256 is captured on first download by re-running the
// script after a version bump — the failure prints actual vs
// expected so you can paste the new hash.
//
// Sources:
//   - fzf:   github.com/junegunn/fzf releases (official upstream)
//   - aria2: github.com/asdo92/aria2-static-builds (third-party
//            static builds; aria2 upstream only ships source +
//            Windows binaries, so this is the cleanest static
//            Linux source available). 724-star repo, current with
//            upstream 1.37.0.
const DEPS = [
	{
		name: 'fzf',
		version: '0.62.0',
		archiveName: 'fzf-0.62.0-linux_amd64.tar.gz',
		url: 'https://github.com/junegunn/fzf/releases/download/v0.62.0/fzf-0.62.0-linux_amd64.tar.gz',
		sha256: '64b56dd484a2317d5f04c28ac0791b36807f034adb419209ad39fb6637255794',
		binary: 'fzf',
		// fzf's Linux tarball is flat — fzf sits at the archive root.
		archivePath: 'fzf',
		tarFlag: '-xzf',
	},
	{
		name: 'aria2',
		version: '1.37.0',
		archiveName: 'aria2-1.37.0-linux-gnu-64bit-build1.tar.bz2',
		url: 'https://github.com/asdo92/aria2-static-builds/releases/download/v1.37.0/aria2-1.37.0-linux-gnu-64bit-build1.tar.bz2',
		sha256: '80c0a04aabaedf1f3cf8ec77861547823b7ecc317fb61220b06c28edf97bb964',
		binary: 'aria2c',
		// aria2's tarball nests the binary under a versioned dir.
		archivePath: 'aria2-1.37.0-linux-gnu-64bit-build1/aria2c',
		tarFlag: '-xjf',
	},
];

const cacheDir = path.join(electronDir, '.linux-deps-cache');
const stagedBinDir = path.join(electronDir, 'build-resources', 'linux', 'bin');

// Dev-mode parity: the Rust backend's `AppState::build` looks for
// bundled deps under `<resource_dir>/bin`, where `resource_dir` is
// the directory holding the backend exe. In dev that's
// `gui/backend/target/<profile>/`, so dropping the deps there makes
// the dev loop work without polluting global PATH.
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
 * the cached archive path. Cache hits are reused; mismatches trigger
 * a redownload.
 */
async function downloadOnce(dep) {
	const cached = path.join(cacheDir, dep.archiveName);
	if (existsSync(cached) && statSync(cached).size > 0) {
		const got = await sha256(cached);
		if (got === dep.sha256) {
			console.log(`[fetch-linux-deps] cache hit: ${cached}`);
			return cached;
		}
		console.warn(`[fetch-linux-deps] cached ${dep.name} checksum mismatch — redownloading`);
		await rm(cached);
	}
	mkdirSync(cacheDir, { recursive: true });
	console.log(`[fetch-linux-deps] downloading: ${dep.url}`);
	const resp = await fetch(dep.url, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status} for ${dep.url}`);
	await pipeline(resp.body, createWriteStream(cached));
	const got = await sha256(cached);
	if (got !== dep.sha256) {
		throw new Error(
			`SHA-256 mismatch for ${dep.archiveName}\n` +
				`  expected: ${dep.sha256}\n` +
				`  got:      ${got}\n` +
				`If upstream rotated the asset (or this is a first-run version bump),` +
				` recompute and update DEPS[${dep.name}].sha256.`,
		);
	}
	console.log(`[fetch-linux-deps] verified ${dep.archiveName}`);
	return cached;
}

/**
 * Extract a single named entry from a tar archive into `destDir`.
 * GNU tar handles both gzip (`-xzf`) and bzip2 (`-xjf`) natively on
 * any Linux build host.
 */
function extractEntry(archive, archivePath, destDir, tarFlag) {
	return new Promise((resolve, reject) => {
		const proc = spawn('tar', [tarFlag, archive, '-C', destDir, archivePath], {
			stdio: 'inherit',
		});
		proc.on('error', reject);
		proc.on('exit', (code) => {
			if (code === 0) resolve();
			else reject(new Error(`tar ${tarFlag} '${archivePath}' from '${archive}' exited ${code}`));
		});
	});
}

/**
 * Stage one dep: download the archive, extract just the binary,
 * `chmod +x` it (tar preserves the source mode but the binary needs
 * to stay executable across hosts), and flatten into the bin dir.
 * Then mirror into the dev-mode cargo target dirs that exist on disk.
 */
async function stageDep(dep) {
	const archive = await downloadOnce(dep);

	await mkdir(stagedBinDir, { recursive: true });
	const stagedBinary = path.join(stagedBinDir, dep.binary);
	if (existsSync(stagedBinary)) await rm(stagedBinary);

	const scratchDir = path.join(cacheDir, `extract-${dep.name}`);
	await mkdir(scratchDir, { recursive: true });
	console.log(`[fetch-linux-deps] extracting ${dep.binary} from ${dep.archiveName}`);
	await extractEntry(archive, dep.archivePath, scratchDir, dep.tarFlag);

	const extractedBinary = path.join(scratchDir, dep.archivePath);
	if (!existsSync(extractedBinary)) {
		throw new Error(`expected ${extractedBinary} after extracting ${dep.archiveName}`);
	}
	await copyFile(extractedBinary, stagedBinary);
	// Force executable bit; tar preserves source perms but we don't
	// want to depend on that across hosts. 0o755 matches Linux
	// convention for binaries.
	// eslint-disable-next-line no-bitwise
	const { chmod } = await import('node:fs/promises');
	await chmod(stagedBinary, 0o755);
	console.log(`[fetch-linux-deps] staged → ${stagedBinary}`);

	for (const devDir of devTargetBinDirs) {
		const profileDir = path.dirname(devDir);
		if (!existsSync(profileDir)) continue;
		await mkdir(devDir, { recursive: true });
		const devBinary = path.join(devDir, dep.binary);
		if (existsSync(devBinary)) await rm(devBinary);
		await copyFile(stagedBinary, devBinary);
		await chmod(devBinary, 0o755);
		console.log(`[fetch-linux-deps] dev copy → ${devBinary}`);
	}
}

async function main() {
	for (const dep of DEPS) {
		console.log(`[fetch-linux-deps] === ${dep.name} ${dep.version} ===`);
		await stageDep(dep);
	}
	console.log(
		`[fetch-linux-deps] done — ${DEPS.map((d) => `${d.name} ${d.version}`).join(', ')} staged for Linux packaging`,
	);
}

main().catch((err) => {
	console.error('[fetch-linux-deps] failed:', err.message);
	process.exit(1);
});
