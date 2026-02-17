import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { Miniflare } from "miniflare";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workerBuildPath = path.resolve(__dirname, "../test-worker/build");

describe("figment2-cloudflare-workers", () => {
  /** @type {Miniflare} */
  let miniflare;

  before(async () => {
    miniflare = new Miniflare({
      scriptPath: path.join(workerBuildPath, "worker", "shim.mjs"),
      modules: true,
      modulesRules: [
        { type: "ESModule", include: ["**/*.js"], fallthrough: true },
        { type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true },
      ],
      compatibilityDate: "2025-01-01",
      bindings: {
        // Plain vars.
        API_BASE_URL: "https://api.example.com/v1",
        MAX_RETRIES: "3",
        // Simulated secret (not in wrangler.toml [vars]).
        API_KEY: "super-secret-key",
      },
    });
  });

  after(async () => {
    await miniflare.dispose();
  });

  it("reads all fields and uppercases binding names", async () => {
    const response = await miniflare.dispatchFetch("http://localhost/full");
    assert.equal(response.status, 200);

    const body = await response.json();
    assert.equal(body.api_base_url, "https://api.example.com/v1");
    assert.equal(body.api_key, "super-secret-key");
    assert.equal(body.max_retries, "3");
  });

  it("skips missing bindings (partial config with Option fields)", async () => {
    const response = await miniflare.dispatchFetch("http://localhost/partial");
    assert.equal(response.status, 200);

    const body = await response.json();
    assert.equal(body.api_base_url, "https://api.example.com/v1");
    assert.equal(body.api_key, "super-secret-key");
    assert.equal(body.missing_field, null);
  });

  it("extracts a single field", async () => {
    const response = await miniflare.dispatchFetch("http://localhost/single");
    assert.equal(response.status, 200);

    const body = await response.json();
    assert.equal(body.api_base_url, "https://api.example.com/v1");
  });

  it("supports custom profiles", async () => {
    const response = await miniflare.dispatchFetch("http://localhost/profile");
    assert.equal(response.status, 200);

    const body = await response.json();
    assert.equal(body.api_base_url, "https://api.example.com/v1");
  });

  it("fails extraction when required fields have no bindings", async () => {
    // Separate Miniflare instance with no bindings at all.
    const emptyMiniflare = new Miniflare({
      scriptPath: path.join(workerBuildPath, "worker", "shim.mjs"),
      modules: true,
      modulesRules: [
        { type: "ESModule", include: ["**/*.js"], fallthrough: true },
        { type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true },
      ],
      compatibilityDate: "2025-01-01",
      bindings: {},
    });

    try {
      const response = await emptyMiniflare.dispatchFetch(
        "http://localhost/missing-all",
      );
      assert.equal(response.status, 200);

      const body = await response.json();
      assert.equal(body.error, true);
      assert.ok(body.message.length > 0, "error message should be non-empty");
    } finally {
      await emptyMiniflare.dispose();
    }
  });
});
