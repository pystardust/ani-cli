// Post-build step: swap electron-builder's bundled AppImage runtime
// (appimage-12.0.1, dlopens libfuse.so.2) with the modern type2-runtime
// (links libfuse3, which Ubuntu 24.04+ ships by default — Tauri's
// AppImage uses the same family of runtime, which is why those ran on
// this machine without an extra `apt install`).
//
// We don't rebuild the squashfs payload; we just re-attach it to a new
// runtime header. The "AppImage" file is literally an ELF runtime
// followed by a squashfs blob, so:
//
//   new.AppImage = new-runtime-elf || squashfs(old.AppImage[elfEnd:])
//
// The squashfs starts at the end of the ELF section header table —
// computable from the ELF64 header alone, no FUSE involved.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const electronDir = path.resolve(__dirname, '..');

// Continuous release of the official type2-runtime. Recent commits
// link libfuse3, which Ubuntu 24.04+ ships by default. Pinning to
// `continuous` keeps us current; for reproducible builds we can later
// swap to a tagged release commit.
const RUNTIME_URL =
	'https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64';
const cacheDir = path.join(electronDir, '.appimage-cache');
const runtimePath = path.join(cacheDir, 'runtime-x86_64');
const distDir = path.join(electronDir, 'dist');

/**
 * Download the modern runtime once, cache it under .appimage-cache/.
 * Subsequent runs are offline.
 */
async function ensureRuntime() {
	if (fs.existsSync(runtimePath) && fs.statSync(runtimePath).size > 0) return;
	fs.mkdirSync(cacheDir, { recursive: true });
	console.log(`[repack] downloading runtime: ${RUNTIME_URL}`);
	const resp = await fetch(RUNTIME_URL, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status}`);
	await pipeline(resp.body, createWriteStream(runtimePath));
	fs.chmodSync(runtimePath, 0o755);
}

/**
 * Compute the byte offset where the squashfs payload begins in an
 * AppImage. Equal to the byte that follows the ELF section header
 * table — `e_shoff + e_shnum * e_shentsize`.
 */
function squashfsOffset(filePath) {
	const fd = fs.openSync(filePath, 'r');
	try {
		const header = Buffer.alloc(64);
		fs.readSync(fd, header, 0, 64, 0);
		const magic = header.subarray(0, 4);
		if (magic[0] !== 0x7f || magic[1] !== 0x45 || magic[2] !== 0x4c || magic[3] !== 0x46) {
			throw new Error('not an ELF file');
		}
		const cls = header[4];
		if (cls !== 2) throw new Error(`only ELF64 supported, got class=${cls}`);
		const shoff = Number(header.readBigUInt64LE(0x28));
		const shentsize = header.readUInt16LE(0x3a);
		const shnum = header.readUInt16LE(0x3c);
		return shoff + shentsize * shnum;
	} finally {
		fs.closeSync(fd);
	}
}

/**
 * Replace the old AppImage's ELF runtime with the modern one. Streams
 * the squashfs through to keep memory usage low even for fat AppImages.
 */
async function repack(appimage) {
	const offset = squashfsOffset(appimage);
	const fileSize = fs.statSync(appimage).size;
	const payloadBytes = fileSize - offset;
	console.log(
		`[repack] ${path.basename(appimage)}: squashfs at offset ${offset}, ${payloadBytes} payload bytes`
	);

	const tmp = appimage + '.repack-tmp';
	const out = fs.createWriteStream(tmp);
	out.write(fs.readFileSync(runtimePath));
	const reader = fs.createReadStream(appimage, { start: offset });
	await pipeline(reader, out);

	fs.chmodSync(tmp, 0o755);
	fs.renameSync(tmp, appimage);
}

async function main() {
	if (!fs.existsSync(distDir)) {
		throw new Error(`dist/ not found — run electron-builder first`);
	}
	await ensureRuntime();
	const candidates = fs
		.readdirSync(distDir)
		.filter((f) => f.endsWith('.AppImage'))
		.map((f) => path.join(distDir, f));
	if (candidates.length === 0) {
		throw new Error(`no .AppImage files in ${distDir}`);
	}
	for (const c of candidates) await repack(c);
	console.log(`[repack] done — ${candidates.length} AppImage(s) repacked with type2-runtime`);
}

main().catch((err) => {
	console.error('[repack] failed:', err.message);
	process.exit(1);
});
