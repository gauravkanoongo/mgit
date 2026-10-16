# mgit Documentation & Repository Index

This page is the starting point for the project documentation. It summarizes each document,
links to its main sections, and maps the source tree to the codebase responsibilities.

## Documentation

| Document | Summary | Main sections |
| --- | --- | --- |
| [README.md](./README.md) | User-facing overview, features, CLI usage and options, builds, tests, supported targets, and license. Treat this as the entry point for using mgit. | [Features](./README.md#features) · [Usage](./README.md#usage) · [Building locally](./README.md#building-locally) · [Supported targets](./README.md#multi-target-os--architecture-support) · [License](./README.md#license) |
| [plan.md](./plan.md) | Product and modernization plan: argument separation, async execution, deterministic output, automation, discovery, and technical direction. Includes default behavior and the implemented opt-in inline UI. | [Argument model](./plan.md#1-disambiguation-strategy-the-single-rule-approach) · [Async execution and ordered output](./plan.md#2-fully-async-architecture-tokio-with-guaranteed-in-order-sequential-output) · [Automation](./plan.md#3-making-mgit-ai-agent--automated-workflow-friendly) · [Discovery](./plan.md#4-modernizing-directory-traversal--discovery) · [Inline UI](./plan.md#2-fully-async-architecture-tokio-with-guaranteed-in-order-sequential-output) |
| [implementation_plan.md](./implementation_plan.md) | Detailed phased implementation and test plan for the core system: argument parsing, discovery, output, execution, and end-to-end integration. Some items describe the original plan; check the source and tests for actual implementation status. | [Phase overview](./implementation_plan.md#phase-structure-overview) · [Phase 1: arguments](./implementation_plan.md#phase-1-toolchain-modernization--argument-parser-engine) · [Phase 2: discovery](./implementation_plan.md#phase-2-repository-discovery-engine--pruning) · [Phase 3: output](./implementation_plan.md#phase-3-in-order-output-streaming--formatters) · [Phase 4: execution](./implementation_plan.md#phase-4-async-process-execution-engine-tokio) · [Phase 5: integration](./implementation_plan.md#phase-5-pipeline-integration-exit-codes--end-to-end-tests) |
| [low_level_design.md](./low_level_design.md) | Technical blueprint for modules, data structures, execution, output, and behavior. Includes the implemented opt-in inline UI architecture. | [System architecture](./low_level_design.md#1-system-architecture-overview) · [Module specifications](./low_level_design.md#2-module-specifications--data-structures) · [Output and inline UI](./low_level_design.md#24-module-output-srcoutputrs) |
| [ui-deliberations.md](./ui-deliberations.md) | Records the UI goal, alternatives, selected inline TUI, progress model, output/memory constraints, exit behavior, implementation choices, and remaining platform validation. | [User goal](./ui-deliberations.md#user-goal) · [Alternatives](./ui-deliberations.md#alternatives-considered) · [Selected direction](./ui-deliberations.md#selected-direction-opt-in-inline-tui) · [Progress](./ui-deliberations.md#progress-model) · [Output and memory](./ui-deliberations.md#output-and-memory-model) · [Decisions and validation](./ui-deliberations.md#decisions-and-validation-status) |
| [todo.md](./todo.md) | Historical tracker for completed Phases 1–5 and their feature/test breakdown, followed by inline-UI implementation status and remaining validation tasks. | [Overall progress](./todo.md#overall-progress) · [Detailed task breakdown](./todo.md#detailed-task-breakdown) · [Inline UI](./todo.md#optional-inline-output-ui) |

### Which document should I read?

- **Using mgit:** start with [README.md](./README.md).
- **Understanding current architecture:** read [low_level_design.md](./low_level_design.md), then inspect the source.
- **Using the inline terminal UI:** read [ui-deliberations.md](./ui-deliberations.md) and the UI section in [low_level_design.md](./low_level_design.md).
- **Checking immediate implementation work:** see [todo.md](./todo.md).
- **Reviewing the broader modernization or original phased plan:** see [plan.md](./plan.md) and [implementation_plan.md](./implementation_plan.md). Their checklists are design/history references and may not reflect current code status.

## Repository structure

```text
mgit/
├── .github/                 GitHub configuration and automation
├── src/
│   ├── args.rs              CLI argument parsing and mgit configuration
│   ├── discovery.rs         Repository traversal and discovery
│   ├── engine.rs            Async Git subprocess execution
│   ├── main.rs              Application entry point and pipeline wiring
│   ├── output.rs            Ordered output, formatters, and summary reporting
│   └── ui.rs                Optional inline progress/output interface
├── tests/
│   └── e2e_tests.rs         End-to-end tests of the compiled CLI
├── Cargo.toml               Package metadata and Rust dependencies
├── Cargo.lock               Locked dependency versions
├── README.md                User guide and project overview
├── plan.md                  Enhancement and modernization plan
├── implementation_plan.md   Detailed phased implementation/test plan
├── low_level_design.md      Technical architecture and module design
├── ui-deliberations.md      UI options, decisions, constraints, and validation status
├── todo.md                  Historical implementation tracker and current tasks
├── LICENSE                  Project license
└── .gitignore               Git ignore rules
```

`target/` is generated by Cargo builds and is not source documentation.

## Current versus planned

The source provides repository discovery, asynchronous Git execution, ordered output, text/JSON
formatting, and Ctrl+C cancellation. The inline progress/output UI is opt-in through `--tui`;
see [ui-deliberations.md](./ui-deliberations.md) and
[low_level_design.md](./low_level_design.md#24-module-output-srcoutputrs).
