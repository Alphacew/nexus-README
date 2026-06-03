import { test } from 'node:test';
import assert from 'node:assert';
import { promises as fs } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
process.env.NODE_ENV = 'test';
import { main } from '../src/cli.js';

const filename = fileURLToPath(import.meta.url);
const dir = dirname(filename);

test('CLI: execution flow creates file and performs atomic swap', async () => {
  const dummyOutput = join(dir, 'DUMMY_README.md');
  const tempOutput = join(dir, '.DUMMY_README.md.tmp');

  // Ensure clean slate
  await fs.unlink(dummyOutput).catch(() => {});
  await fs.unlink(tempOutput).catch(() => {});

  // Remove API key so agent falls back to local stubs during test
  const oldApiKey = process.env['GEMINI_API_KEY'];
  delete process.env['GEMINI_API_KEY'];

  try {
    await main([
      '-w', join(dir, '../../'),
      '-o', dummyOutput,
      '-e', 'orchestrator/node_modules,orchestrator/dist',
    ]);

    // Check that output file exists
    const stats = await fs.stat(dummyOutput);
    assert.ok(stats.isFile(), 'Output README file should be created');

    const content = await fs.readFile(dummyOutput, 'utf8');
    assert.ok(content.includes('<!-- NEXUS_START:OVERVIEW -->'), 'Overview should exist');
    assert.ok(content.includes('<!-- NEXUS_START:GRAPH -->'), 'Graph should exist');
    assert.ok(content.includes('<!-- NEXUS_START:ARCHITECTURE -->'), 'Architecture should exist');
    assert.ok(content.includes('<!-- NEXUS_START:REFERENCE -->'), 'Reference should exist');
    assert.ok(content.includes('```mermaid'), 'Mermaid block should exist');

    // Staging file should NOT exist anymore (atomic rename)
    await assert.rejects(
      fs.stat(tempOutput),
      (err: any) => err.code === 'ENOENT',
      'Temporary staging file should have been renamed and no longer exists'
    );
  } finally {
    // Restore env and cleanup
    process.env['GEMINI_API_KEY'] = oldApiKey;
    await fs.unlink(dummyOutput).catch(() => {});
  }
});
