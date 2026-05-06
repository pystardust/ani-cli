// Ratchet check: fail CI if any coverage metric drops below the
// baseline pinned in `coverage-baseline.json`. Reads layer-specific
// coverage outputs from the standard locations each language's
// coverage tooling writes.
//
// Usage:
//   node tools/check-coverage-baseline.mjs [--update]
//
// `--update` writes the current numbers back to the baseline file —
// run it deliberately after intentional changes to test scope.
//
// Layers checked:
//   - rust:     gui/backend/lcov.info  (cargo-llvm-cov --lcov)
//   - frontend: gui/frontend/coverage/coverage-summary.json (vitest --coverage)
//   - bash:     coverage/bash/{pure,network}/index.js (kcov)
//   - crap:     coverage/crap-summary.json (tools/crap-score.mjs --json)
//
// Each layer's metrics live in coverage-baseline.json. A missing
// coverage file for a layer is treated as "couldn't measure" — the
// layer is skipped with a warning rather than failing the gate, so
// partial CI runs (e.g. only frontend changed) still pass.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const baselinePath = path.join(repoRoot, 'coverage-baseline.json');

const update = process.argv.includes('--update');

/** @type {{ layer: string, metric: string, current: number, baseline: number, ok: boolean }[]} */
const results = [];
const measured = {};

function readJson(p) {
	return JSON.parse(fs.readFileSync(p, 'utf-8'));
}

/**
 * Parse an lcov.info file and return aggregate line / function / region
 * percentages. cargo-llvm-cov writes both LF/LH (lines found / hit) and
 * FNF/FNH (functions found / hit) and BRF/BRH (branches found / hit).
 */
function summarizeLcov(file) {
	const text = fs.readFileSync(file, 'utf-8');
	let LF = 0,
		LH = 0,
		FNF = 0,
		FNH = 0,
		BRF = 0,
		BRH = 0;
	for (const raw of text.split('\n')) {
		const line = raw.trim();
		if (line.startsWith('LF:')) LF += Number(line.slice(3));
		else if (line.startsWith('LH:')) LH += Number(line.slice(3));
		else if (line.startsWith('FNF:')) FNF += Number(line.slice(4));
		else if (line.startsWith('FNH:')) FNH += Number(line.slice(4));
		else if (line.startsWith('BRF:')) BRF += Number(line.slice(4));
		else if (line.startsWith('BRH:')) BRH += Number(line.slice(4));
	}
	return {
		lines_pct: LF ? (100 * LH) / LF : 0,
		functions_pct: FNF ? (100 * FNH) / FNF : 0,
		regions_pct: BRF ? (100 * BRH) / BRF : 0
	};
}

function checkRust(baseline) {
	const lcov = path.join(repoRoot, 'gui/backend/lcov.info');
	if (!fs.existsSync(lcov)) {
		console.warn(`[skip] rust: ${lcov} not found`);
		return;
	}
	const cur = summarizeLcov(lcov);
	measured.rust = round(cur);
	// `regions_pct` from lcov is actually branch coverage, which
	// cargo-llvm-cov doesn't emit by default. Stick to lines +
	// functions; both are reliably present.
	for (const k of ['lines_pct', 'functions_pct']) {
		results.push({
			layer: 'rust',
			metric: k,
			current: cur[k],
			baseline: baseline[k],
			ok: cur[k] + 0.01 >= baseline[k]
		});
	}
}

function checkFrontend(baseline) {
	const summaryPath = path.join(repoRoot, 'gui/frontend/coverage/coverage-summary.json');
	if (!fs.existsSync(summaryPath)) {
		console.warn(`[skip] frontend: ${summaryPath} not found`);
		return;
	}
	const summary = readJson(summaryPath);
	const total = summary.total ?? {};
	const cur = {
		lines_pct: total.lines?.pct ?? 0,
		functions_pct: total.functions?.pct ?? 0,
		statements_pct: total.statements?.pct ?? 0,
		branches_pct: total.branches?.pct ?? 0
	};
	measured.frontend = round(cur);
	for (const k of ['lines_pct', 'functions_pct', 'statements_pct', 'branches_pct']) {
		results.push({
			layer: 'frontend',
			metric: k,
			current: cur[k],
			baseline: baseline[k],
			ok: cur[k] + 1e-6 >= baseline[k]
		});
	}
}

/**
 * Parse a kcov index.js header line (a `var header = {…}` blob)
 * and return the covered / instrumented line counts.
 */
function summarizeKcov(indexJs) {
	if (!fs.existsSync(indexJs)) return null;
	const text = fs.readFileSync(indexJs, 'utf-8');
	const m = text.match(/^var header = ({[^\n]+});/m);
	if (!m) return null;
	// kcov emits a JS object literal — keys are already double-quoted
	// but it has a trailing comma before `}`, which JSON.parse rejects.
	// Strip those before parsing.
	const cleaned = m[1].replace(/,(\s*})/g, '$1');
	const obj = JSON.parse(cleaned);
	return { covered: Number(obj.covered ?? 0), instrumented: Number(obj.instrumented ?? 0) };
}

function checkBash(baseline) {
	const sets = [
		['pure', 'pure_covered_min'],
		['network', 'network_covered_min']
	];
	const cur = {};
	for (const [name, _] of sets) {
		const idx = path.join(repoRoot, 'coverage/bash', name, 'index.js');
		const c = summarizeKcov(idx);
		if (c) cur[name] = c;
	}
	if (Object.keys(cur).length === 0) {
		console.warn(`[skip] bash: no kcov outputs under coverage/bash/`);
		return;
	}
	measured.bash = cur;
	for (const [name, key] of sets) {
		if (!cur[name]) continue;
		results.push({
			layer: 'bash',
			metric: name + '_covered',
			current: cur[name].covered,
			baseline: baseline[key],
			ok: cur[name].covered >= baseline[key]
		});
	}
}

function round(o) {
	const out = {};
	for (const [k, v] of Object.entries(o)) {
		out[k] = typeof v === 'number' ? Math.round(v * 100) / 100 : v;
	}
	return out;
}

function checkCrap(baseline) {
	const summaryPath = path.join(repoRoot, 'coverage/crap-summary.json');
	if (!fs.existsSync(summaryPath)) {
		console.warn(`[skip] crap: ${summaryPath} not found`);
		return;
	}
	const cur = readJson(summaryPath);
	measured.crap = { max: cur.max, p95: cur.p95, high_risk: cur.high_risk };
	// CRAP is "lower is better": we ratchet against ceilings, not floors.
	// A 1.0 fudge stops floating-point round-trips from flagging a no-op
	// re-measurement as a regression.
	results.push({
		layer: 'crap',
		metric: 'max_le',
		current: cur.max,
		baseline: baseline.max_le,
		ok: cur.max <= baseline.max_le + 1.0,
		ceiling: true
	});
	results.push({
		layer: 'crap',
		metric: 'p95_le',
		current: cur.p95,
		baseline: baseline.p95_le,
		ok: cur.p95 <= baseline.p95_le + 1.0,
		ceiling: true
	});
	results.push({
		layer: 'crap',
		metric: 'high_risk_le',
		current: cur.high_risk,
		baseline: baseline.high_risk_le,
		ok: cur.high_risk <= baseline.high_risk_le,
		ceiling: true
	});
}

const baseline = readJson(baselinePath);
checkRust(baseline.rust);
checkFrontend(baseline.frontend);
checkBash(baseline.bash);
checkCrap(baseline.crap);

if (update) {
	const next = { ...baseline };
	if (measured.rust) next.rust = measured.rust;
	if (measured.frontend) next.frontend = measured.frontend;
	if (measured.bash) {
		next.bash = {
			pure_covered_min: measured.bash.pure?.covered ?? baseline.bash.pure_covered_min,
			network_covered_min: measured.bash.network?.covered ?? baseline.bash.network_covered_min
		};
	}
	if (measured.crap) {
		next.crap = {
			...baseline.crap,
			max_le: Math.ceil(measured.crap.max),
			p95_le: Math.ceil(measured.crap.p95),
			high_risk_le: measured.crap.high_risk
		};
	}
	fs.writeFileSync(baselinePath, JSON.stringify(next, null, '\t') + '\n');
	console.log(`[update] wrote refreshed baseline to ${baselinePath}`);
	process.exit(0);
}

const fails = results.filter((r) => !r.ok);
const widths = { layer: 10, metric: 20, current: 10, baseline: 10 };
function pad(s, w) {
	const str = String(s);
	return str + ' '.repeat(Math.max(0, w - str.length));
}
console.log(`\n${pad('layer', widths.layer)}${pad('metric', widths.metric)}${pad('current', widths.current)}${pad('baseline', widths.baseline)}status`);
console.log('-'.repeat(70));
for (const r of results) {
	const cur = typeof r.current === 'number' && r.current % 1 !== 0 ? r.current.toFixed(2) : String(r.current);
	const bas = typeof r.baseline === 'number' && r.baseline % 1 !== 0 ? r.baseline.toFixed(2) : String(r.baseline);
	console.log(
		pad(r.layer, widths.layer) +
			pad(r.metric, widths.metric) +
			pad(cur, widths.current) +
			pad(bas, widths.baseline) +
			(r.ok ? 'ok' : 'BELOW BASELINE')
	);
}

if (fails.length > 0) {
	console.error(`\n${fails.length} metric(s) dropped below baseline. Re-run with --update if intentional.`);
	process.exit(1);
}
console.log('\nall measured metrics meet or exceed baseline.');
