import { type CodebaseTopology } from './schema.js';

/**
 * Generates a Mermaid.js flowchart mapping out module dependencies.
 */
export function generateMermaidGraph(topology: CodebaseTopology): string {
  const lines: string[] = [];
  lines.push('flowchart LR');

  // Group modules under their primary top-level directory names
  const groups = new Map<string, typeof topology.modules>();

  for (const module of topology.modules) {
    const parts = module.filePath.split(/[/\\]/);
    const topLevel = parts.length > 1 ? parts[0]! : 'Root';

    if (!groups.has(topLevel)) {
      groups.set(topLevel, []);
    }
    groups.get(topLevel)!.push(module);
  }

  // Write subgraph blocks
  for (const [topLevel, modules] of groups.entries()) {
    const subgraphId = topLevel.replace(/[^a-zA-Z0-9]/g, '_');
    lines.push(`    subgraph ${subgraphId} ["${topLevel}"]`);

    for (const module of modules) {
      const nodeId = module.filePath.replace(/[^a-zA-Z0-9]/g, '_');
      const lang = module.language.toLowerCase();
      let shape = '';

      if (lang === 'python') {
        shape = `${nodeId}(["${module.filePath}"])`;
      } else if (lang === 'typescript' || lang === 'javascript') {
        shape = `${nodeId}("${module.filePath}")`;
      } else if (lang === 'rust') {
        shape = `${nodeId}[["${module.filePath}"]]`;
      } else {
        shape = `${nodeId}["${module.filePath}"]`;
      }

      lines.push(`        ${shape}`);
    }

    lines.push('    end');
  }

  lines.push('');

  // Draw directed arrows pointing toward imports
  for (const module of topology.modules) {
    const nodeId = module.filePath.replace(/[^a-zA-Z0-9]/g, '_');

    for (const depPath of module.internalDependencies) {
      const depNodeId = depPath.replace(/[^a-zA-Z0-9]/g, '_');
      lines.push(`    ${nodeId} --> ${depNodeId}`);
    }
  }

  return lines.join('\n');
}
