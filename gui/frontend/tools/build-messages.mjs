// Merge per-namespace JSON bundles in `messages/<locale>/*.json` into
// the single `messages/<locale>.json` file Paraglide's plugin reads.
//
// We author per-namespace (player, settings, errors, …) so adding a
// surface doesn't pile keys into one giant file; Paraglide's
// plugin-message-format only supports a single source file per
// locale, so this step glues them together. Each namespace file's
// stem becomes the key prefix:
//
//   messages/en/player.json   { "skip_op": "Skip Opening" }
//   messages/en/settings.json { "title": "Settings" }
//                       │
//                       ▼  build-messages.mjs
//   messages/en.json          { "$schema": "...",
//                                "player_skip_op": "Skip Opening",
//                                "settings_title": "Settings" }
//
// Paraglide then compiles `m.player_skip_op()` / `m.settings_title()`.
//
// Gitignored output: `messages/<locale>.json` is regenerated; the
// per-namespace files in `messages/<locale>/` are the source of truth.

import { readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const messagesRoot = join(__dirname, '..', 'messages');

if (!existsSync(messagesRoot)) {
	console.error(`build-messages: ${messagesRoot} does not exist`);
	process.exit(1);
}

const locales = readdirSync(messagesRoot, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

if (locales.length === 0) {
	console.error('build-messages: no locale directories under messages/');
	process.exit(1);
}

for (const locale of locales) {
	const dir = join(messagesRoot, locale);
	const namespaceFiles = readdirSync(dir).filter((f) => f.endsWith('.json'));
	const merged = { $schema: 'https://inlang.com/schema/inlang-message-format' };
	for (const file of namespaceFiles.sort()) {
		const ns = file.replace(/\.json$/, '');
		const raw = JSON.parse(readFileSync(join(dir, file), 'utf-8'));
		for (const [key, value] of Object.entries(raw)) {
			// Skip the per-namespace `$schema` if authors include one.
			if (key === '$schema') continue;
			const flatKey = `${ns}_${key}`;
			if (Object.prototype.hasOwnProperty.call(merged, flatKey)) {
				console.error(
					`build-messages: duplicate key "${flatKey}" (locale ${locale}, namespace ${ns})`
				);
				process.exit(1);
			}
			merged[flatKey] = value;
		}
	}
	const out = join(messagesRoot, `${locale}.json`);
	writeFileSync(out, JSON.stringify(merged, null, '\t') + '\n');
	console.log(`build-messages: wrote ${out} (${Object.keys(merged).length - 1} keys)`);
}
