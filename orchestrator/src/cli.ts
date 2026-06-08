import { promises as fs } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import * as readline from 'readline/promises';
import { stdin as input, stdout as output } from 'process';
import { runParserBinary } from './runner.js';
import { runAgentPipeline } from './agent.js';
import { patchReadme } from './lock.js';
import { generateMermaidGraph } from './graph.js';
import dotenv from 'dotenv';

dotenv.config();

async function ensureApiKey() {
  if (process.env['GEMINI_API_KEY']) {
    return;
  }

  if (!process.stdin.isTTY || process.env['NODE_ENV'] === 'test') {
    return;
  }
  
  const envPath = resolve(process.cwd(), '.env');
  const rl = readline.createInterface({ input, output });
  const key = await rl.question('GEMINI_API_KEY not found. Please enter your Gemini API key: ');
  rl.close();
  
  if (key.trim()) {
    await fs.appendFile(envPath, `\nGEMINI_API_KEY=${key.trim()}\n`, { mode: 0o600 });
    try {
      await fs.chmod(envPath, 0o600);
    } catch (err: any) {
      // Ignored if unsupported or already correct
    }
    process.env['GEMINI_API_KEY'] = key.trim();
    console.log('API key saved to .env file.');
  } else {
    console.warn('No API key provided. Running in STUB fallback mode.');
  }
}


/**
 * Main CLI orchestrator logic.
 */
export async function main(args: string[]): Promise<void> {
  await ensureApiKey();

  let workspaceDir = '.';
  let outputFilePath = 'README.md';
  let excludeList: string[] = [];

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (!arg) continue;

    if (arg === '--workspace' || arg === '-w') {
      workspaceDir = args[++i] || '.';
    } else if (arg === '--output' || arg === '-o') {
      outputFilePath = args[++i] || 'README.md';
    } else if (arg === '--exclude' || arg === '-e') {
      const paths = args[++i] || '';
      excludeList = paths.split(',').map((p) => p.trim()).filter(Boolean);
    } else if (!arg.startsWith('-') && i === 0) {
      workspaceDir = arg;
    }
  }

  const absWorkspaceDir = resolve(workspaceDir);
  const absOutputFilePath = resolve(outputFilePath);

  // 1. Run static analysis parser
  const topology = await runParserBinary({
    workspaceDir: absWorkspaceDir,
    exclude: excludeList,
  });

  // 2. Execute Prompt Pipeline
  const generated = await runAgentPipeline(topology);

  // 3. Generate Mermaid graph and wrap in markdown fence blocks
  const graphRaw = generateMermaidGraph(topology);
  const graphMarkdown = `\`\`\`mermaid\n${graphRaw}\n\`\`\``;

  // 4. Read current markdown target if it exists
  let existingContent = '';
  try {
    existingContent = await fs.readFile(absOutputFilePath, 'utf8');
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
      throw err;
    }
  }

  // 5. Parse section locks and merge content
  const patchedContent = patchReadme(existingContent, {
    OVERVIEW: generated.OVERVIEW,
    GRAPH: graphMarkdown,
    ARCHITECTURE: generated.ARCHITECTURE,
    REFERENCE: generated.REFERENCE,
  });

  // 5. Perform atomic overwrite using staging file
  const targetDir = dirname(absOutputFilePath);
  const baseName = absOutputFilePath.split('/').pop() || 'README.md';
  const tempFilePath = resolve(targetDir, `.${baseName}.tmp`);

  await fs.writeFile(tempFilePath, patchedContent, 'utf8');
  await fs.rename(tempFilePath, absOutputFilePath);

  console.log(`Successfully updated documentation at: ${absOutputFilePath}`);
}

// Auto-run if executed directly
const nodePath = process.argv[1];
if (nodePath) {
  const filename = fileURLToPath(import.meta.url);
  // Check if current file was invoked directly (compiled or via tsx)
  if (
    resolve(nodePath) === resolve(filename) || 
    nodePath.endsWith('tsx') || 
    nodePath.includes('bin/cli')
  ) {
    // Only execute if this appears to be the direct target entrypoint
    const isDirectRun = process.argv[2] && (
      process.argv[2].includes('cli.ts') || 
      process.argv[2].includes('cli.js')
    );
    // tsx passes the script path as the second argv element, check if it's us
    if (isDirectRun || resolve(nodePath) === resolve(filename)) {
      main(process.argv.slice(isDirectRun ? 3 : 2)).catch((err) => {
        console.error('Fatal CLI Error:', err);
        process.exit(1);
      });
    }
  }
}
