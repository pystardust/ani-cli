/**
 * Custom ESLint rule: every visitor-facing string in a Svelte template
 * must be routed through Paraglide (`m.foo()`), not embedded as a
 * literal. Catches the two main slip surfaces:
 *
 *   1. Raw text in a template — `<button>Save</button>`. Flagged via
 *      the `SvelteText` AST node.
 *   2. Literal attribute values on user-facing attributes —
 *      `aria-label="Close"`, `placeholder="Search"`, `title="…"`,
 *      `alt="…"`. Flagged via `SvelteAttribute` whose first child is
 *      a `SvelteLiteral`.
 *
 * Heuristic: a value is "translatable" when it has ≥2 ASCII letters
 * and either contains a space or ends with sentence punctuation.
 * Pure numbers, single tokens, IDs, hex colours, URL fragments,
 * mono-character glyphs (✓, ▾, ▸…) are skipped.
 *
 * Escape hatch: a Svelte comment containing `i18n-ignore` on the
 * preceding line silences the rule for that node — preserves the
 * arch sniffer's escape hatch convention.
 *
 * The rule is intentionally noisy on suspicion: false positives are
 * cheap (one-line ignore comment) but a missed translation slips
 * past every check we have.
 */

const USER_FACING_ATTRS = new Set([
	'aria-label',
	'aria-description',
	'aria-roledescription',
	'aria-placeholder',
	'aria-valuetext',
	'title',
	'placeholder',
	'alt',
	'label'
]);

/** Cheap heuristic for "looks like English text we'd want translated". */
function isTranslatable(raw) {
	if (typeof raw !== 'string') return false;
	const value = raw.trim();
	if (value.length < 2) return false;
	// Single-character glyphs / dingbats — common as decorative button content.
	if (value.length === 1) return false;
	// All-numeric, all-punct, ID-like (kebab/snake/camel without spaces).
	const letters = (value.match(/[A-Za-z]/g) ?? []).length;
	if (letters < 2) return false;
	// Pure tokens with no separator are usually identifiers / class names /
	// CSS values — not translatable copy.
	const hasSpace = /\s/.test(value);
	const endsSentence = /[.!?…]$/.test(value);
	const startsCapital = /^[A-Z]/.test(value);
	if (!hasSpace && !endsSentence && !startsCapital) return false;
	// Common safe singletons that look capital but are tokens.
	const tokenAllowlist = new Set([
		'EN',
		'JP',
		'OK',
		'CC',
		'UTC',
		'AM',
		'PM',
		'TV',
		'MP4',
		'HLS',
		'PiP',
		'OP',
		'ED'
	]);
	if (tokenAllowlist.has(value)) return false;
	return true;
}

/** Walk up the AST to find a parent of `type`. */
function findParent(node, type) {
	let cur = node.parent;
	while (cur) {
		if (cur.type === type) return cur;
		cur = cur.parent;
	}
	return null;
}

/** Is the node inside a Svelte comment-marked block? Walks back a few
 *  tokens looking for `i18n-ignore` in the previous comment. */
function hasIgnoreMarker(node, sourceCode) {
	const before = sourceCode.getTokensBefore(node, { count: 8, includeComments: true });
	for (const t of before) {
		if (t.type === 'HTMLComment' && t.value && t.value.includes('i18n-ignore')) return true;
		if (t.type === 'Block' && t.value && t.value.includes('i18n-ignore')) return true;
		if (t.type === 'Line' && t.value && t.value.includes('i18n-ignore')) return true;
	}
	return false;
}

export default {
	meta: {
		type: 'problem',
		docs: {
			description:
				'disallow hardcoded English text in Svelte templates — route every visitor-facing string through Paraglide (m.foo())'
		},
		schema: [],
		messages: {
			text: 'Hardcoded text "{{ value }}" — wrap in Paraglide (m.foo()) or annotate with <!-- i18n-ignore -->.',
			attr: 'Hardcoded text in `{{ attr }}` ("{{ value }}") — wrap in Paraglide (m.foo()) or annotate with <!-- i18n-ignore -->.'
		}
	},
	create(context) {
		const sourceCode = context.sourceCode ?? context.getSourceCode();

		return {
			SvelteText(node) {
				if (!isTranslatable(node.value)) return;
				if (hasIgnoreMarker(node, sourceCode)) return;
				// Inside a <style>, <script>, or {@const} block — not user-visible.
				if (findParent(node, 'SvelteStyleElement')) return;
				if (findParent(node, 'SvelteScriptElement')) return;
				context.report({
					node,
					messageId: 'text',
					data: { value: node.value.trim().slice(0, 50) }
				});
			},
			SvelteAttribute(node) {
				const attrName = typeof node.key?.name === 'string' ? node.key.name : null;
				if (!attrName || !USER_FACING_ATTRS.has(attrName)) return;
				if (!Array.isArray(node.value) || node.value.length !== 1) return;
				const child = node.value[0];
				if (!child || child.type !== 'SvelteLiteral') return;
				if (!isTranslatable(child.value)) return;
				if (hasIgnoreMarker(node, sourceCode)) return;
				context.report({
					node,
					messageId: 'attr',
					data: {
						attr: attrName,
						value: String(child.value).trim().slice(0, 50)
					}
				});
			}
		};
	}
};
