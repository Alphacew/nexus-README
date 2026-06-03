# Codebase Documentation

<!-- NEXUS_START:OVERVIEW -->
### Core Project Overview
This project, **nexus-readme**, is designed to automate documentation scanning.
It includes entrypoints like: None found.
<!-- NEXUS_END:OVERVIEW -->

<!-- NEXUS_START:GRAPH -->
```mermaid
flowchart LR
    subgraph core_parser ["core-parser"]
        core_parser_src_crawler_rs[["core-parser/src/crawler.rs"]]
        core_parser_src_main_rs[["core-parser/src/main.rs"]]
        core_parser_src_parser_engine_rs[["core-parser/src/parser_engine.rs"]]
    end
    subgraph orchestrator ["orchestrator"]
        orchestrator_src_agent_ts("orchestrator/src/agent.ts")
        orchestrator_src_cli_ts("orchestrator/src/cli.ts")
        orchestrator_src_graph_ts("orchestrator/src/graph.ts")
        orchestrator_src_lock_ts("orchestrator/src/lock.ts")
        orchestrator_src_runner_ts("orchestrator/src/runner.ts")
        orchestrator_src_schema_ts("orchestrator/src/schema.ts")
        orchestrator_tests_agent_test_ts("orchestrator/tests/agent.test.ts")
        orchestrator_tests_cli_test_ts("orchestrator/tests/cli.test.ts")
        orchestrator_tests_graph_test_ts("orchestrator/tests/graph.test.ts")
        orchestrator_tests_lock_test_ts("orchestrator/tests/lock.test.ts")
        orchestrator_tests_runner_test_ts("orchestrator/tests/runner.test.ts")
    end

    core_parser_src_main_rs --> core_parser_src_crawler_rs
    core_parser_src_main_rs --> core_parser_src_parser_engine_rs
```
<!-- NEXUS_END:GRAPH -->

<!-- NEXUS_START:ARCHITECTURE -->
### Quickstart Guide
1. **Build the Rust static core:**
   ```bash
   cargo build --release
   ```
2. **Execute the TypeScript Orchestrator:**
   ```bash
   npm run build && npm run test
   ```
<!-- NEXUS_END:ARCHITECTURE -->

<!-- NEXUS_START:REFERENCE -->
### Module Reference Table
| Module File | Language | Exports |
| --- | --- | --- |
| `core-parser/src/crawler.rs` | rust | `WorkspaceCrawler` (struct), `new` (function), `crawl` (function), `CrawlerVisitor` (struct), `CrawlerVisitorBuilder` (struct) |
| `core-parser/src/main.rs` | rust | `Args` (struct), `GitMetadata` (struct), `TopologyModule` (struct), `CodebaseTopology` (struct) |
| `core-parser/src/parser_engine.rs` | rust | `ExportInfo` (struct), `ParsedModule` (struct), `ASTAnalyzer` (struct), `new` (function), `detect_language` (function), `analyze_file` (function) |
| `orchestrator/src/agent.ts` | typescript | `AgentPipelineOptions` (interface), `GenerationResult` (interface), `runAgentPipeline` (function) |
| `orchestrator/src/cli.ts` | typescript | `main` (function) |
| `orchestrator/src/graph.ts` | typescript | `generateMermaidGraph` (function) |
| `orchestrator/src/lock.ts` | typescript | `patchReadme` (function) |
| `orchestrator/src/runner.ts` | typescript | `RunnerOptions` (interface), `BinaryRunnerError` (class), `resolveBinaryPath` (function), `runParserBinary` (function) |
| `orchestrator/src/schema.ts` | typescript | `CodebaseTopology` (type) |
| `orchestrator/tests/agent.test.ts` | typescript | None |
| `orchestrator/tests/cli.test.ts` | typescript | None |
| `orchestrator/tests/graph.test.ts` | typescript | None |
| `orchestrator/tests/lock.test.ts` | typescript | None |
| `orchestrator/tests/runner.test.ts` | typescript | None |
<!-- NEXUS_END:REFERENCE -->
