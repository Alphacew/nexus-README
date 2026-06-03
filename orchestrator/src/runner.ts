import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';
import { existsSync } from 'fs';
import { CodebaseTopologySchema, type CodebaseTopology } from './schema.js';

export interface RunnerOptions {
  binPath?: string;
  workspaceDir: string;
  exclude?: string[];
}

export class BinaryRunnerError extends Error {
  constructor(
    message: string,
    public readonly exitCode: number | null,
    public readonly stdout: string,
    public readonly stderr: string
  ) {
    super(message);
    this.name = 'BinaryRunnerError';
  }
}

/**
 * Resolves the path to the parser binary dynamically.
 * Priority:
 * 1. Explicitly provided options.binPath
 * 2. NEXUS_PARSER_PATH environment variable
 * 3. Relative path relative to the current file location (dist/runner.js)
 */
export function resolveBinaryPath(
  customPath?: string,
  platform: string = process.platform,
  arch: string = process.arch
): string {
  if (customPath) {
    return resolve(customPath);
  }

  if (process.env['NEXUS_PARSER_PATH']) {
    return resolve(process.env['NEXUS_PARSER_PATH']);
  }

  const filename = fileURLToPath(import.meta.url);
  let currentDir = dirname(filename);
  const isWindows = platform === 'win32';
  const binName = isWindows ? 'core-parser.exe' : 'core-parser';
  const packageKey = `@nexus-readme/cli-${platform}-${arch}`;

  // 1. Check scoped optional packages under node_modules
  const maxDepth = 10;
  let nodeModulesDir = currentDir;
  for (let i = 0; i < maxDepth; i++) {
    const potentialBin = resolve(nodeModulesDir, 'node_modules', packageKey, 'bin', binName);
    if (existsSync(potentialBin)) {
      return potentialBin;
    }
    const parent = dirname(nodeModulesDir);
    if (parent === nodeModulesDir) {
      break;
    }
    nodeModulesDir = parent;
  }

  // 2. Traverse upwards to dynamically locate the local development core-parser root folder
  let localDir = currentDir;
  for (let i = 0; i < maxDepth; i++) {
    const potentialCoreParser = resolve(localDir, 'core-parser');
    if (existsSync(potentialCoreParser)) {
      const releaseBin = resolve(potentialCoreParser, `target/release/${binName}`);
      if (existsSync(releaseBin)) {
        return releaseBin;
      }
      const debugBin = resolve(potentialCoreParser, `target/debug/${binName}`);
      if (existsSync(debugBin)) {
        return debugBin;
      }
    }
    const parent = dirname(localDir);
    if (parent === localDir) {
      break;
    }
    localDir = parent;
  }

  // Absolute fallback path matching workspace structure
  const fallbackDebug = resolve('/home/ace/nexus-readme/core-parser/target/debug/core-parser');
  return existsSync(fallbackDebug) ? fallbackDebug : resolve('/home/ace/nexus-readme/core-parser/target/release/core-parser');
}

/**
 * Spawns the core-parser binary concurrently, collects stdout/stderr buffers safely,
 * and validates the resulting JSON topology against our Zod contract.
 */
export function runParserBinary(options: RunnerOptions): Promise<CodebaseTopology> {
  return new Promise((resolvePromise, reject) => {
    const binPath = resolveBinaryPath(options.binPath);
    const args: string[] = [options.workspaceDir];

    if (options.exclude && options.exclude.length > 0) {
      args.push('--exclude', options.exclude.join(','));
    }

    const child = spawn(binPath, args);

    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];

    child.stdout.on('data', (chunk: Buffer) => {
      stdoutChunks.push(chunk);
    });

    child.stderr.on('data', (chunk: Buffer) => {
      stderrChunks.push(chunk);
    });

    child.on('error', (err) => {
      reject(new Error(`Failed to start child process: ${err.message}`));
    });

    child.on('close', (code) => {
      const stdoutStr = Buffer.concat(stdoutChunks).toString('utf8');
      const stderrStr = Buffer.concat(stderrChunks).toString('utf8');

      if (code !== 0) {
        reject(
          new BinaryRunnerError(
            `Parser binary exited with code ${code}. Stderr: ${stderrStr.trim()}`,
            code,
            stdoutStr,
            stderrStr
          )
        );
        return;
      }

      // Defensive JSON parsing wrapper
      let parsedJson: unknown;
      try {
        parsedJson = JSON.parse(stdoutStr);
      } catch (err: any) {
        reject(
          new BinaryRunnerError(
            `Failed to parse binary stdout as JSON: ${err.message}`,
            code,
            stdoutStr,
            stderrStr
          )
        );
        return;
      }

      // Strict Zod contract validation
      const validationResult = CodebaseTopologySchema.safeParse(parsedJson);
      if (!validationResult.success) {
        const formattedErrors = validationResult.error.format();
        reject(
          new Error(
            `CodebaseTopology validation failed: ${JSON.stringify(
              formattedErrors,
              null,
              2
            )}`
          )
        );
        return;
      }

      resolvePromise(validationResult.data);
    });
  });
}
