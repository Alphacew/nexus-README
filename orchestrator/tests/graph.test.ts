import { test } from 'node:test';
import assert from 'node:assert';
import { generateMermaidGraph } from '../src/graph.js';
import { type CodebaseTopology } from '../src/schema.js';

test('Graph Generator: LR flowchart, subgraphs, language shapes, and edges', () => {
  const mockTopology: CodebaseTopology = {
    projectName: 'my-project',
    entryPoints: [],
    dependencies: {},
    modules: [
      {
        filePath: 'core-parser/src/main.rs',
        language: 'rust',
        exports: [],
        internalDependencies: ['core-parser/src/crawler.rs'],
      },
      {
        filePath: 'core-parser/src/crawler.rs',
        language: 'rust',
        exports: [],
        internalDependencies: [],
      },
      {
        filePath: 'orchestrator/src/cli.ts',
        language: 'typescript',
        exports: [],
        internalDependencies: ['orchestrator/src/lock.ts'],
      },
      {
        filePath: 'orchestrator/src/lock.ts',
        language: 'typescript',
        exports: [],
        internalDependencies: [],
      },
      {
        filePath: 'scripts/run.py',
        language: 'python',
        exports: [],
        internalDependencies: [],
      },
      {
        filePath: 'index.js',
        language: 'javascript',
        exports: [],
        internalDependencies: [],
      },
    ],
    environmentVariables: [],
    gitMetadata: { latestCommits: [] },
  };

  const output = generateMermaidGraph(mockTopology);

  // Check graph declaration
  assert.ok(output.startsWith('flowchart LR'), 'Must start with flowchart LR');

  // Check subgraphs
  assert.ok(output.includes('subgraph core_parser ["core-parser"]'), 'Should have core-parser subgraph');
  assert.ok(output.includes('subgraph orchestrator ["orchestrator"]'), 'Should have orchestrator subgraph');
  assert.ok(output.includes('subgraph scripts ["scripts"]'), 'Should have scripts subgraph');
  assert.ok(output.includes('subgraph Root ["Root"]'), 'Should group root files under Root subgraph');

  // Check language-specific shapes
  assert.ok(output.includes('core_parser_src_main_rs[["core-parser/src/main.rs"]]'), 'Rust module should have double square bracket shape');
  assert.ok(output.includes('orchestrator_src_cli_ts("orchestrator/src/cli.ts")'), 'TypeScript module should have round bracket shape');
  assert.ok(output.includes('scripts_run_py(["scripts/run.py"])'), 'Python module should have round rectangle shape');
  assert.ok(output.includes('index_js("index.js")'), 'JavaScript module should have round bracket shape');

  // Check dependency arrows pointing to imports
  assert.ok(output.includes('core_parser_src_main_rs --> core_parser_src_crawler_rs'), 'Main should point to crawler');
  assert.ok(output.includes('orchestrator_src_cli_ts --> orchestrator_src_lock_ts'), 'CLI should point to lock');
});
