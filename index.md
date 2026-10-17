# mgit Documentation Index

Start here for project guidance, architecture, history, and current decisions.

## Documents

| Document | Purpose |
| --- | --- |
| [README.md](./README.md) | User-facing feature summary, command-line options, build, test, and platform notes. |
| [plan.md](./plan.md) | Product direction, argument model, execution and output strategy, and maintained terminal UX policy. |
| [low_level_design.md](./low_level_design.md) | Module responsibilities, data models, execution, ordered output, and exit codes. |
| [implementation_plan.md](./implementation_plan.md) | Original phased implementation and test plan; read as historical planning context, not current task status. |
| [branch-retirement.md](./branch-retirement.md) | Comparison of the stable and experimental milestones, carry-forward decisions, and local branch archival record. |
| [todo.md](./todo.md) | Append-only historical task tracker with dated follow-up sections. |

## Quick navigation

- **Using mgit:** [README.md](./README.md)
- **Understanding runtime behavior:** [low_level_design.md](./low_level_design.md)
- **Current product direction:** [plan.md](./plan.md) and [branch-retirement.md](./branch-retirement.md)
- **Historical implementation progress:** [todo.md](./todo.md)

## Repository structure

```text
mgit/
├── src/
│   ├── args.rs              CLI parsing and mgit configuration
│   ├── discovery.rs         Repository traversal and discovery
│   ├── engine.rs            Async Git subprocess execution
│   ├── main.rs              Application entry point and pipeline wiring
│   └── output.rs            Ordered output, formatters, and summary reporting
├── tests/
│   └── e2e_tests.rs         End-to-end CLI tests
├── Cargo.toml               Package metadata and dependencies
├── Cargo.lock               Locked dependency versions
├── README.md                User guide and project overview
├── plan.md                  Product and modernization direction
├── implementation_plan.md   Original phased implementation/test plan
├── low_level_design.md      Technical architecture and module design
├── branch-retirement.md     Branch comparison and carry-forward decisions
├── index.md                 Documentation index
└── todo.md                  Append-only implementation tracker
```

Generated build output such as `target/` is not source documentation.
