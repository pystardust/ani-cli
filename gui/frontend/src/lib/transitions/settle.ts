// Tile-grid entrance / exit transitions used by /anime/[id] and /play/[id]
// to make ep tiles fade up + scale + de-blur into place under cubicOut. With a
// per-index delay this gives a staggered "settle" feel as a page of tiles
// lands. Reduced motion drops to a flat fade.
//
// css(t, u): for `in:`, t goes 0→1 and u = 1−t. So at t=0 the tile starts at
// opacity 0, scaled to 0.9, translated +28px below its final position, and
// blurred by 8px; it eases out to its rest state by t=1. settleOut mirrors
// the easing curve on the way out (drop + fade + soft blur), and uses a
// shorter duration so old tiles clear before new ones finish settling.

import { cubicOut } from 'svelte/easing';

interface SettleOpts {
	delay?: number;
	duration?: number;
}

function reducedMotion(): boolean {
	return (
		typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
	);
}

export function settle(_node: Element, { delay = 0, duration = 620 }: SettleOpts = {}) {
	const reduced = reducedMotion();
	return {
		delay,
		duration: reduced ? 0 : duration,
		easing: cubicOut,
		css: (t: number, u: number) =>
			reduced
				? `opacity: ${t};`
				: `opacity: ${t}; transform: translateY(${u * 28}px) scale(${0.9 + t * 0.1}); filter: blur(${u * 8}px);`
	};
}

export function settleOut(_node: Element, { delay = 0, duration = 320 }: SettleOpts = {}) {
	const reduced = reducedMotion();
	return {
		delay,
		duration: reduced ? 0 : duration,
		easing: cubicOut,
		// For `out:` Svelte runs t from 1→0; u = 1−t. Mirror the in: shape
		// but drop upward (negative translateY) so it doesn't feel like the
		// same gesture playing in reverse.
		css: (t: number, u: number) =>
			reduced
				? `opacity: ${t};`
				: `opacity: ${t}; transform: translateY(${u * -16}px) scale(${0.94 + t * 0.06}); filter: blur(${u * 4}px);`
	};
}
