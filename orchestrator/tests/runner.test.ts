import { test } from "node:test";
import assert from "node:assert";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import { CodebaseTopologySchema } from "../src/schema.js";
import {
  runParserBinary,
  BinaryRunnerError,
  resolveBinaryPath,
} from "../src/runner.js";

const filename = fileURLToPath(import.meta.url);
const dir = dirname(filename);
const workspaceDir = join(dir, "../../");

test("Zod Schema validation", () => {
  const validPayload = {
    projectName: "nexus-readme",
    entryPoints: ["src/main.rs"],
    dependencies: {
      clap: "4.0",
    },
    modules: [
      {
        filePath: "src/main.rs",
        language: "rust",
        exports: [
          {
            name: "main",
            type: "function",
            description: "Main function",
          },
        ],
        internalDependencies: [],
      },
    ],
    environmentVariables: ["API_KEY"],
    gitMetadata: {
      latestCommits: ["Initial commit"],
    },
  };

  const result = CodebaseTopologySchema.safeParse(validPayload);
  assert.ok(result.success, "Schema should accept valid payloads");

  // Verify type mapping
  if (result.success) {
    assert.strictEqual(result.data.projectName, "nexus-readme");
    assert.strictEqual(result.data.modules[0]?.exports[0]?.name, "main");
  }

  // Schema rejection on missing fields
  const invalidPayload = {
    projectName: "broken",
    modules: [],
  };
  const invalidResult = CodebaseTopologySchema.safeParse(invalidPayload);
  assert.strictEqual(
    invalidResult.success,
    false,
    "Schema should reject partial payloads",
  );
});

test("Parser binary integration test", async () => {
  const binPath = resolveBinaryPath();

  try {
    const topology = await runParserBinary({
      binPath,
      workspaceDir,
      exclude: ["orchestrator/node_modules"],
    });

    assert.ok(
      topology.projectName === 'nexus-readme' || topology.projectName === 'app',
      "Project name should match folder",
    );
    assert.ok(Array.isArray(topology.modules), "Modules should be an array");
    assert.ok(topology.modules.length > 0, "Should find modules in workspace");

    // Verify main module structure
    const mainModule = topology.modules.find((m) =>
      m.filePath.endsWith("src/main.rs"),
    );
    assert.ok(mainModule, "Should find main.rs module");
    assert.strictEqual(mainModule.language, "rust");
    assert.ok(mainModule.exports.length > 0, "Exports should not be empty");
  } catch (err: any) {
    assert.fail(`Integration test failed: ${err.message}`);
  }
});

test("Parser binary handles non-existent workspace directory", async () => {
  const binPath = resolveBinaryPath();
  const nonExistentDir = join(workspaceDir, "non_existent_folder_xyz");

  await assert.rejects(
    runParserBinary({
      binPath,
      workspaceDir: nonExistentDir,
    }),
    (err: any) => {
      // It should either fail to spawn or return an exit code indicating error
      return err instanceof Error;
    },
    "Should reject on invalid workspace directory",
  );
});

test("Parser binary error handling for invalid binary path", async () => {
  const badBinPath = join(workspaceDir, "non_existent_binary");

  await assert.rejects(
    runParserBinary({
      binPath: badBinPath,
      workspaceDir,
    }),
    (err: any) => {
      return (
        err.message.includes("Failed to start child process") ||
        err instanceof BinaryRunnerError
      );
    },
    "Should reject on invalid binary path",
  );
});

test("Parser binary path resolver respects platform and architecture overrides", () => {
  // Test Windows x64 resolution
  const pathWin = resolveBinaryPath(undefined, "win32", "x64");
  assert.ok(
    pathWin.endsWith("core-parser.exe") || pathWin.includes("core-parser"),
    "Should map to core-parser.exe or fallback to development target",
  );

  // Test Linux x64 resolution
  const pathLinux = resolveBinaryPath(undefined, "linux", "x64");
  assert.ok(
    pathLinux.includes("core-parser"),
    "Should resolve to core-parser or fallback to development target",
  );

  // Test macOS ARM64 resolution
  const pathMac = resolveBinaryPath(undefined, "darwin", "arm64");
  assert.ok(
    pathMac.includes("core-parser"),
    "Should resolve to core-parser or fallback to development target",
  );
});
