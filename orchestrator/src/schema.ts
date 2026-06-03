import { z } from 'zod';

export const ExportItemSchema = z.object({
  name: z.string(),
  type: z.string(),
  description: z.string().optional(),
  meta: z.record(z.string(), z.any()).optional(),
});

export const ModuleSchema = z.object({
  filePath: z.string(),
  language: z.string(),
  exports: z.array(ExportItemSchema),
  internalDependencies: z.array(z.string()),
});

export const CodebaseTopologySchema = z.object({
  projectName: z.string(),
  entryPoints: z.array(z.string()),
  dependencies: z.record(z.string(), z.string()),
  modules: z.array(ModuleSchema),
  environmentVariables: z.array(z.string()),
  gitMetadata: z.object({
    latestCommits: z.array(z.string()),
  }),
});

// Infer compile-time types from our runtime schema
export type CodebaseTopology = z.infer<typeof CodebaseTopologySchema>;
