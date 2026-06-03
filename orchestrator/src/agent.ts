import { type CodebaseTopology } from './schema.js';

export interface AgentPipelineOptions {
  apiKey?: string;
  useMock?: boolean;
  mockResponses?: {
    tier1?: string;
    tier2?: string;
    tier3?: string;
  };
}

export interface GenerationResult {
  OVERVIEW: string;
  ARCHITECTURE: string;
  REFERENCE: string;
}

/**
 * Invokes the Gemini API generateContent endpoint with the chosen model.
 */
async function callGemini(
  model: string,
  prompt: string,
  apiKey: string
): Promise<string> {
  const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${apiKey}`;
  
  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        contents: [
          {
            parts: [
              {
                text: prompt,
              },
            ],
          },
        ],
      }),
    });

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'No response body');
      throw new Error(`Gemini API returned status ${response.status}: ${errorText}`);
    }

    const data = await response.json() as any;
    const text = data?.candidates?.[0]?.content?.parts?.[0]?.text;
    
    if (typeof text !== 'string') {
      throw new Error(`Unexpected Gemini response format: ${JSON.stringify(data)}`);
    }

    return text.trim();
  } catch (err: any) {
    throw new Error(`Failed to query Gemini model ${model}: ${err.message}`);
  }
}

/**
 * Runs the 3-Tier agentic prompt pipeline using Hybrid Model Routing and Cumulative Prompt Chaining.
 */
export async function runAgentPipeline(
  topology: CodebaseTopology,
  options: AgentPipelineOptions = {}
): Promise<GenerationResult> {
  const apiKey = options.apiKey || process.env['GEMINI_API_KEY'];
  const useMock = options.useMock || !apiKey;

  if (!apiKey && !options.useMock) {
    console.warn(
      'WARNING: No GEMINI_API_KEY found in environment or options. Running in STUB fallback mode.'
    );
  }

  // Define Mock Responses (either passed in, or default stubs)
  const defaultStubs = {
    tier1: `### Core Project Overview
This project, **${topology.projectName}**, is designed to automate documentation scanning.
It includes entrypoints like: ${topology.entryPoints.join(', ') || 'None found'}.`,
    tier2: `### Quickstart Guide
1. **Build the Rust static core:**
   \`\`\`bash
   cargo build --release
   \`\`\`
2. **Execute the TypeScript Orchestrator:**
   \`\`\`bash
   npm run build && npm run test
   \`\`\``,
    tier3: `### Module Reference Table
| Module File | Language | Exports |
| --- | --- | --- |
${topology.modules.map(m => `| \`${m.filePath}\` | ${m.language} | ${m.exports.map(e => `\`${e.name}\` (${e.type})`).join(', ') || 'None'} |`).join('\n')}`
  };

  const mockTier1 = options.mockResponses?.tier1 || defaultStubs.tier1;
  const mockTier2 = options.mockResponses?.tier2 || defaultStubs.tier2;
  const mockTier3 = options.mockResponses?.tier3 || defaultStubs.tier3;

  // --- Tier 1: Intent & Persona Discovery (gemini-3.5-flash) ---
  const tier1Model = 'gemini-3.5-flash';
  const tier1Prompt = `You are a world-class technical writer.
Analyze this codebase topology and deduce the project's macro purpose, target persona, and core value proposition.
Output a high-level project description in clean, structured markdown. Do not include markdown codeblocks or system summaries.

Codebase Topology:
${JSON.stringify(topology, null, 2)}`;

  let tier1Output = '';
  if (useMock) {
    tier1Output = mockTier1;
  } else {
    tier1Output = await callGemini(tier1Model, tier1Prompt, apiKey!);
  }

  // --- Tier 2: Quickstart & Technical Synthesis (gemini-3.5-flash) ---
  // Cumulative Chaining: Append Tier 1 output as context.
  const tier2Model = 'gemini-3.5-flash';
  const tier2Prompt = `You are an elite Senior Engineer.
Generate a deterministic setup and synthesis guide based on the codebase topology and Project Overview.
Write complete, copy-pasteable installation scripts, environment variables, build/test commands, and quickstart examples.

Codebase Topology:
${JSON.stringify(topology, null, 2)}

Project Overview (Tier 1 Discovery):
${tier1Output}

Output your response in clean markdown.`;

  let tier2Output = '';
  if (useMock) {
    tier2Output = mockTier2;
  } else {
    tier2Output = await callGemini(tier2Model, tier2Prompt, apiKey!);
  }

  // --- Tier 3: API/Usage Reference Mapping (gemini-2.5-flash) ---
  // Cumulative Chaining: Append Tier 1 and Tier 2 outputs as context.
  const tier3Model = 'gemini-2.5-flash';
  const tier3Prompt = `You are an expert technical writer.
Generate a comprehensive markdown API and module reference layout mapping files, languages, and exported symbols.
Avoid generic descriptions. Map out the files and their public symbols accurately.

Codebase Topology:
${JSON.stringify(topology, null, 2)}

Context Summary:
---
Overview (Tier 1):
${tier1Output}
---
Quickstart & Setup (Tier 2):
${tier2Output}
---

Output a clean, tabular module reference mapping in markdown.`;

  let tier3Output = '';
  if (useMock) {
    tier3Output = mockTier3;
  } else {
    tier3Output = await callGemini(tier3Model, tier3Prompt, apiKey!);
  }

  return {
    OVERVIEW: tier1Output,
    ARCHITECTURE: tier2Output,
    REFERENCE: tier3Output,
  };
}
