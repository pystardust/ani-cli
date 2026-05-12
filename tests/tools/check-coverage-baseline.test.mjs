// Pin behavior of tools/check-coverage-baseline.mjs's --update flags.
//
// The script supports two writer modes:
//   --update       refresh all baselines (rust/frontend/bash/crap)
//   --update-crap  refresh only the CRAP ceilings, leaving the
//                  coverage-floor metrics (rust/frontend/bash) at
//                  their existing values.
//
// The bug these tests guard against: running `--update-crap` on a
// repo where lcov.info / coverage-summary.json / kcov outputs are
// also present silently re-wrote rust/frontend/bash baselines from
// the current measurements. A user re-running coverage to tighten
// CRAP could thereby lock in a degraded coverage floor.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../..');
const scriptUnderTest = path.join(repoRoot, 'tools/check-coverage-baseline.mjs');

/**
 * Stage a fixture repo in a tmpdir: copy the script in, write a
 * baseline + measurement inputs for every layer, then return the
 * tmpdir + a runner that invokes the script.
 */
function stageFixtureRepo() {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'cov-baseline-test-'));
	const toolsDir = path.join(tmpDir, 'tools');
	fs.mkdirSync(toolsDir, { recursive: true });
	fs.copyFileSync(scriptUnderTest, path.join(toolsDir, 'check-coverage-baseline.mjs'));

	const baseline = {
		rust: { lines_pct: 80, functions_pct: 75 },
		frontend: { lines_pct: 70, functions_pct: 65, statements_pct: 70, branches_pct: 60 },
		bash: { pure_covered_min: 100, network_covered_min: 50 },
		crap: { max_le: 100, p95_le: 50, high_risk_le: 5 }
	};
	fs.writeFileSync(
		path.join(tmpDir, 'coverage-baseline.json'),
		JSON.stringify(baseline, null, '\t') + '\n'
	);

	// Rust lcov.info: numbers above baseline so writing them in would
	// move the floor up — the bug-canary signal.
	const lcov = [
		'TN:',
		'SF:src/foo.rs',
		'LF:100',
		'LH:95',
		'FNF:20',
		'FNH:19',
		'end_of_record'
	].join('\n');
	fs.mkdirSync(path.join(tmpDir, 'gui/backend'), { recursive: true });
	fs.writeFileSync(path.join(tmpDir, 'gui/backend/lcov.info'), lcov);

	// Frontend coverage-summary.json: also above baseline.
	const summary = {
		total: {
			lines: { pct: 90 },
			functions: { pct: 85 },
			statements: { pct: 90 },
			branches: { pct: 80 }
		}
	};
	fs.mkdirSync(path.join(tmpDir, 'gui/frontend/coverage'), { recursive: true });
	fs.writeFileSync(
		path.join(tmpDir, 'gui/frontend/coverage/coverage-summary.json'),
		JSON.stringify(summary)
	);

	// Bash kcov index.js stub for pure + network.
	for (const [name, covered] of [
		['pure', 200],
		['network', 75]
	]) {
		fs.mkdirSync(path.join(tmpDir, `coverage/bash/${name}`), { recursive: true });
		const header = `var header = {"covered": ${covered}, "instrumented": 1000,};\n`;
		fs.writeFileSync(path.join(tmpDir, `coverage/bash/${name}/index.js`), header);
	}

	// Crap-summary.json: tighter than baseline so --update-crap moves it.
	const crap = { max: 80, p95: 40, high_risk: 3 };
	fs.mkdirSync(path.join(tmpDir, 'coverage'), { recursive: true });
	fs.writeFileSync(path.join(tmpDir, 'coverage/crap-summary.json'), JSON.stringify(crap));

	const run = (args) =>
		execFileSync(
			'node',
			[path.join(toolsDir, 'check-coverage-baseline.mjs'), ...args],
			{ cwd: tmpDir, encoding: 'utf-8' }
		);
	const readBaseline = () =>
		JSON.parse(fs.readFileSync(path.join(tmpDir, 'coverage-baseline.json'), 'utf-8'));

	return { tmpDir, run, readBaseline, baseline };
}

test('--update-crap leaves rust/frontend/bash baselines untouched', () => {
	const { run, readBaseline, baseline } = stageFixtureRepo();
	run(['--update-crap']);
	const next = readBaseline();

	assert.deepEqual(
		next.rust,
		baseline.rust,
		'rust baseline must not be rewritten by --update-crap'
	);
	assert.deepEqual(
		next.frontend,
		baseline.frontend,
		'frontend baseline must not be rewritten by --update-crap'
	);
	assert.deepEqual(
		next.bash,
		baseline.bash,
		'bash baseline must not be rewritten by --update-crap'
	);
});

test('--update-crap tightens crap ceilings when the codebase improved', () => {
	const { run, readBaseline } = stageFixtureRepo();
	run(['--update-crap']);
	const next = readBaseline();

	assert.equal(next.crap.max_le, 80, 'max_le tightens to ceil(measured.max)');
	assert.equal(next.crap.p95_le, 40, 'p95_le tightens to ceil(measured.p95)');
	assert.equal(next.crap.high_risk_le, 3, 'high_risk_le tightens to measured');
});

test('--update refreshes every layer including crap', () => {
	const { run, readBaseline, baseline } = stageFixtureRepo();
	run(['--update']);
	const next = readBaseline();

	assert.notDeepEqual(
		next.rust,
		baseline.rust,
		'rust baseline should be refreshed by --update'
	);
	assert.notDeepEqual(
		next.frontend,
		baseline.frontend,
		'frontend baseline should be refreshed by --update'
	);
	assert.notDeepEqual(
		next.bash,
		baseline.bash,
		'bash baseline should be refreshed by --update'
	);
});

test('--update will not lower coverage floors below baseline', () => {
	// Mirrors PR #13's firm-CRAP rule for the other direction: a
	// coverage regression must not be papered over by re-running
	// `--update`. The fixture's measurements (rust lines 95 / functions
	// 95, frontend lines 90 / branches 80, bash 200 / 75) are BELOW
	// the inflated baseline values written here, so a tighten-only
	// `--update` must leave the higher floors in place.
	const { tmpDir, run, readBaseline } = stageFixtureRepo();
	const baselinePath = path.join(tmpDir, 'coverage-baseline.json');
	const inflated = JSON.parse(fs.readFileSync(baselinePath, 'utf-8'));
	inflated.rust = { lines_pct: 99, functions_pct: 99 };
	inflated.frontend = {
		lines_pct: 99,
		functions_pct: 99,
		statements_pct: 99,
		branches_pct: 99
	};
	inflated.bash = { pure_covered_min: 999, network_covered_min: 999 };
	fs.writeFileSync(baselinePath, JSON.stringify(inflated, null, '\t') + '\n');

	run(['--update']);
	const next = readBaseline();

	assert.equal(next.rust.lines_pct, 99, 'rust lines_pct must not regress');
	assert.equal(next.rust.functions_pct, 99, 'rust functions_pct must not regress');
	assert.equal(next.frontend.lines_pct, 99, 'frontend lines_pct must not regress');
	assert.equal(next.frontend.branches_pct, 99, 'frontend branches_pct must not regress');
	assert.equal(next.bash.pure_covered_min, 999, 'bash pure_covered_min must not regress');
	assert.equal(
		next.bash.network_covered_min,
		999,
		'bash network_covered_min must not regress'
	);
});

test('--update still raises coverage floors when the codebase improved', () => {
	// Symmetric to the regression guard: a real improvement must
	// land in the baseline. Fixture's measurements (rust lines 95)
	// are ABOVE the default baseline (rust lines 80), so the
	// resulting floor should be the higher measured value.
	const { run, readBaseline } = stageFixtureRepo();
	run(['--update']);
	const next = readBaseline();

	assert.equal(next.rust.lines_pct, 95, 'rust lines_pct should rise to measured');
	assert.equal(next.rust.functions_pct, 95, 'rust functions_pct should rise to measured');
	assert.equal(next.frontend.lines_pct, 90, 'frontend lines_pct should rise to measured');
	assert.equal(
		next.bash.pure_covered_min,
		200,
		'bash pure_covered_min should rise to measured'
	);
});
