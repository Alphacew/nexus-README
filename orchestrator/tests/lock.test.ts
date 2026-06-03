import { test } from 'node:test';
import assert from 'node:assert';
import { patchReadme } from '../src/lock.js';

test('Lock Engine: generates skeleton from empty string', () => {
  const sections = {
    OVERVIEW: 'Stub Overview',
    GRAPH: 'Stub Graph',
    ARCHITECTURE: 'Stub Arch',
    REFERENCE: 'Stub Ref',
  };

  const output = patchReadme('', sections);

  assert.ok(output.includes('<!-- NEXUS_START:OVERVIEW -->\nStub Overview\n<!-- NEXUS_END:OVERVIEW -->'));
  assert.ok(output.includes('<!-- NEXUS_START:GRAPH -->\nStub Graph\n<!-- NEXUS_END:GRAPH -->'));
  assert.ok(output.includes('<!-- NEXUS_START:ARCHITECTURE -->\nStub Arch\n<!-- NEXUS_END:ARCHITECTURE -->'));
  assert.ok(output.includes('<!-- NEXUS_START:REFERENCE -->\nStub Ref\n<!-- NEXUS_END:REFERENCE -->'));
  assert.ok(output.startsWith('# Codebase Documentation'));
});

test('Lock Engine: updates only inner contents and preserves external text', () => {
  const existing = `# User Manual

This is manual text before.

<!-- NEXUS_START:OVERVIEW -->
Old Overview
<!-- NEXUS_END:OVERVIEW -->

Intermediary manual instructions.

<!-- NEXUS_START:REFERENCE -->
Old Reference
<!-- NEXUS_END:REFERENCE -->

Postscript instructions.
`;

  const sections = {
    OVERVIEW: 'New Overview Content',
    REFERENCE: 'New Reference Table',
  };

  const output = patchReadme(existing, sections);

  assert.ok(output.includes('This is manual text before.'));
  assert.ok(output.includes('Intermediary manual instructions.'));
  assert.ok(output.includes('Postscript instructions.'));
  assert.ok(output.includes('<!-- NEXUS_START:OVERVIEW -->\nNew Overview Content\n<!-- NEXUS_END:OVERVIEW -->'));
  assert.ok(output.includes('<!-- NEXUS_START:REFERENCE -->\nNew Reference Table\n<!-- NEXUS_END:REFERENCE -->'));
  // Old content should be gone
  assert.strictEqual(output.includes('Old Overview'), false);
  assert.strictEqual(output.includes('Old Reference'), false);
});

test('Lock Engine: handles spacing-agnostic HTML comments and OS-specific line endings', () => {
  const existing = `<!--NEXUS_START:OVERVIEW-->
Some old data
<!--   NEXUS_END:OVERVIEW   -->\r
Other content\r
<!--   NEXUS_START:REFERENCE-->
Data
<!--NEXUS_END:REFERENCE   -->`;

  const sections = {
    OVERVIEW: 'Upgraded Overview',
    REFERENCE: 'Upgraded Reference',
  };

  const output = patchReadme(existing, sections);

  assert.ok(output.includes('<!--NEXUS_START:OVERVIEW-->\nUpgraded Overview\n<!--   NEXUS_END:OVERVIEW   -->'));
  assert.ok(output.includes('<!--   NEXUS_START:REFERENCE-->\nUpgraded Reference\n<!--NEXUS_END:REFERENCE   -->'));
  assert.ok(output.includes('Other content'));
});

test('Lock Engine: appends missing sections to the end of existing file', () => {
  const existing = `# My Project
Existing documentation here.`;

  const sections = {
    OVERVIEW: 'Appended Overview',
  };

  const output = patchReadme(existing, sections);

  assert.ok(output.startsWith('# My Project'));
  assert.ok(output.includes('<!-- NEXUS_START:OVERVIEW -->\nAppended Overview\n<!-- NEXUS_END:OVERVIEW -->'));
});
