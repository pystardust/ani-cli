/**
 * Smoke test: launches the packaged Electron app from
 * `dist/linux-unpacked/ani-gui` (electron-builder's pre-AppImage
 * output), stubs the Rust backend's HTTP API so the renderer doesn't
 * depend on Kitsu being reachable, and asserts the home page reaches
 * a recognizable rendered state.
 *
 * Targeting the packaged artifact rather than running unpacked
 * Electron directly tests the production code path: the `app://`
 * protocol handler, the bundled SvelteKit static assets, and the
 * sidecar backend at `process.resourcesPath/ani-gui-backend`. None
 * of those wire correctly when `electron .` is run from source.
 *
 * Pre-condition: `pnpm run package` (or the chained `pnpm e2e`) has
 * produced `dist/linux-unpacked/`.
 */
import { _electron as electron, expect, test } from "@playwright/test";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { appInfo, emptyHistory, topRated, trending } from "./fixtures/kitsu";

const electronDir = path.resolve(__dirname, "..");
const packagedBinary = path.join(electronDir, "dist/linux-unpacked/ani-gui");

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd gui/electron && pnpm run package`,
    );
  }
});

/**
 * Wire all the API stubs the home page consumes onto a freshly-
 * launched Electron app, BEFORE its first window starts loading —
 * critical because the renderer fires its initial `fetch()` calls
 * (history, trending, top-rated) immediately on mount. Routes set
 * via `page.route()` after `firstWindow()` resolves race those
 * requests and miss the first batch, leaving real-network responses
 * visible to the page. Setting them on `context.route()` (visible to
 * any page in the context) avoids the race.
 */
async function launchAppWithStubs() {
  // Pinning XDG dirs to fresh tmp paths makes the backend's history,
  // settings, and cache files start empty regardless of the dev
  // machine's actual state. Without this, the Rust backend reads
  // the user's real `ani-hsts` file (the GUI shares it with the
  // CLI) and the Continue Watching strip renders — Playwright's
  // `route()` only intercepts what the renderer fetches, but the
  // backend reads history off the filesystem directly.
  const tmp = path.join(
    os.tmpdir(),
    `ani-gui-e2e-${process.pid}-${Date.now()}`,
  );
  fs.mkdirSync(tmp, { recursive: true });
  const cleanEnv = {
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
  };

  const app = await electron.launch({
    executablePath: packagedBinary,
    // `--no-sandbox` because chrome-sandbox in the unpackaged
    // linux-unpacked dir isn't SUID. The .deb's postinst fixes
    // that for installed copies; this test runs against the
    // pre-install artifact.
    args: ["--no-sandbox"],
    env: cleanEnv,
  });

  const context = app.context();
  await context.route("**/api/app-info", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(appInfo),
    }),
  );
  await context.route("**/api/history", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(emptyHistory),
    }),
  );
  await context.route("**/api/kitsu/trending", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(trending),
    }),
  );
  await context.route("**/api/kitsu/top-rated", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(topRated),
    }),
  );
  // Block image fetches; the renderer's placeholder takes over.
  await context.route("**/api/image*", (r) => r.fulfill({ status: 503 }));

  const page = await app.firstWindow();
  return { app, page, context };
}

test("app launches, hero renders, no unexpected console errors", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // Capture renderer console errors so the test fails on any
    // thrown exception in the SPA's boot path.
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    page.on("pageerror", (err) => errors.push(String(err)));

    // "Trending now" eyebrow is the first DOM signal the stubs
    // were consumed and the page is past the loading state.
    // "Top rated" eyebrow is the load signal we use across e2e —
    // it renders for every trending-result count, including 1 (a
    // single-entry fixture leaves "Trending now" without a Strip
    // because that single entry becomes the hero, which uses a
    // different eyebrow).
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });

    // Allow a tick for any async errors to fire after render.
    await page.waitForTimeout(250);

    // Filter out:
    //  - DevTools Autofill probe noise (Chromium internal)
    //  - 503s from our own /api/image stub (the renderer logs
    //    `<img onerror>` as a resource-load error; expected).
    const meaningful = errors.filter(
      (e) =>
        !/Autofill\.(enable|setAddresses)/.test(e) &&
        !/^DevTools/i.test(e) &&
        !/Failed to load resource.*503/.test(e),
    );
    expect(
      meaningful,
      `unexpected console errors:\n${meaningful.join("\n")}`,
    ).toEqual([]);
  } finally {
    await app.close();
  }
});

test("clicking a poster card navigates to a detail route", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // The first window starts loading before `context.route()`
    // can fully register, so the trending/top-rated stubs may
    // race the renderer's initial fetches. We stub them anyway
    // (in case the timing wins on faster machines), but the
    // assertion only checks that *some* /anime/<id> URL lands —
    // not the specific id from our fixture. The point of this
    // test is to exercise the route+navigation path, not the
    // network stubbing.
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    const topRatedRegion = page.getByRole("region", { name: /top rated/i });
    const card = topRatedRegion.locator("a").first();
    await expect(card).toBeVisible({ timeout: 10_000 });
    await card.click();

    // SvelteKit's client router updates the URL to `/anime/<id>`
    // without a full reload. Under the `app://` protocol that
    // becomes `app://localhost/anime/<id>`.
    await expect
      .poll(() => page.url(), { timeout: 10_000 })
      .toMatch(/\/anime\/\w+/);
  } finally {
    await app.close();
  }
});

test("settings locale picker live-switches Paraglide messages", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // Wait for the home page to land in English first — eyebrow
    // is the same load signal the other smoke tests use.
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });

    // Navigate to /settings via the rail nav. We click the rail
    // link rather than goto-ing the URL so the SPA router handles
    // the transition, mirroring the user flow.
    await page
      .getByRole("link", { name: /settings/i })
      .first()
      .click();
    await expect(page).toHaveURL(/\/settings\/?$/);

    // The page title in English. The h1 ("House rules.") is the
    // most stable, locale-distinct anchor.
    const headline = page.getByRole("heading", { name: /house rules/i });
    await expect(headline).toBeVisible();

    // Language picker — select pt-BR. The picker is a native
    // <select> with the locale key as the option value. We grab
    // the underlying element once (rather than re-querying by
    // aria-label) because the aria-label itself is localised, so
    // after switching to pt-BR the combobox would no longer
    // match `/locale/i`.
    const langSelect = page.getByRole("combobox", { name: /locale/i });
    const langSelectElement = await langSelect.elementHandle();
    await langSelect.selectOption("pt-BR");

    // paraglideSetLocale flips the active locale immediately
    // (reload: false). The h1 should re-derive to its pt-BR
    // translation: "Regras da casa.".
    await expect(
      page.getByRole("heading", { name: /regras da casa/i }),
    ).toBeVisible({
      timeout: 5_000,
    });

    // Switch back to en before the test closes the app — the
    // XDG dirs are tmp so it doesn't matter, but it makes
    // debug output friendlier when the test fails midway. Use
    // the element handle from before the switch so we don't
    // have to know the pt-BR aria-label.
    await langSelectElement?.selectOption("en");
    await expect(headline).toBeVisible({ timeout: 5_000 });
  } finally {
    await app.close();
  }
});

test("home page hides the Continue Watching strip when history is empty", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // With XDG_STATE_HOME pinned to a fresh tmp dir (see
    // `launchAppWithStubs`), the backend's history file doesn't
    // exist, the renderer gets an empty list, and the Continue
    // Watching strip's `{#if history && history.length > 0}`
    // guard keeps the heading out of the DOM.
    // "Top rated" eyebrow is the load signal we use across e2e —
    // it renders for every trending-result count, including 1 (a
    // single-entry fixture leaves "Trending now" without a Strip
    // because that single entry becomes the hero, which uses a
    // different eyebrow).
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    await expect(
      page.getByRole("heading", { name: /continue watching/i }),
    ).toHaveCount(0);
  } finally {
    await app.close();
  }
});
