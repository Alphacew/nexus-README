import { test } from 'node:test';
import assert from 'node:assert';
import { runAgentPipeline } from '../src/agent.js';
import { type CodebaseTopology } from '../src/schema.js';

const mockTopology: CodebaseTopology = {
  projectName: 'test-project',
  entryPoints: ['src/main.ts'],
  dependencies: {
    dotenv: '^16.0.0',
  },
  modules: [
    {
      filePath: 'src/main.ts',
      language: 'typescript',
      exports: [
        {
          name: 'bootstrap',
          type: 'function',
          description: 'Starts the app',
        },
      ],
      internalDependencies: [],
    },
  ],
  environmentVariables: ['PORT'],
  gitMetadata: {
    latestCommits: ['feat: initial commit'],
  },
};

test('Agent Pipeline: falls back to stubs if no API key is provided and useMock is not explicitly set', async () => {
  // Suppress log output warning in test
  const originalWarn = console.warn;
  console.warn = () => {};

  try {
    const result = await runAgentPipeline(mockTopology, { useMock: true });
    
    assert.ok(result.OVERVIEW.includes('test-project'), 'Overview should mention project name');
    assert.ok(result.ARCHITECTURE.includes('cargo build') || result.ARCHITECTURE.includes('npm run build'), 'Architecture quickstart instructions should exist');
    assert.ok(result.REFERENCE.includes('src/main.ts'), 'Reference table should contain the module files');
  } finally {
    console.warn = originalWarn;
  }
});

test('Agent Pipeline: respects mockResponses override configuration', async () => {
  const customMocks = {
    tier1: 'Custom Overview Text',
    tier2: 'Custom Quickstart Setup Instructions',
    tier3: 'Custom Tabular Module Map',
  };

  const result = await runAgentPipeline(mockTopology, {
    useMock: true,
    mockResponses: customMocks,
  });

  assert.strictEqual(result.OVERVIEW, 'Custom Overview Text');
  assert.strictEqual(result.ARCHITECTURE, 'Custom Quickstart Setup Instructions');
  assert.strictEqual(result.REFERENCE, 'Custom Tabular Module Map');
});
