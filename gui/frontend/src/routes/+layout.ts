// Tauri runs the webview against a static build with no SSR. Disabling
// prerender for everything except `/` means we render index.html and the
// SvelteKit router handles every other route on the client.
export const prerender = false;
export const ssr = false;
export const csr = true;
