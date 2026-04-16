---
number: 39
title: "DD-29: `lykn new` — Project Template Generation"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-04-16
updated: 2026-04-16
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-29: `lykn new` — Project Template Generation

**Status**: Decided
**Date**: 2026-04-16
**Depends on**: DD-28 (workspace convention, `project.json`, CLI)
**Blocks**: Book Ch 1 (Getting Started)

## Summary

`lykn new <name>` generates a ready-to-use lykn project following
the workspace conventions established in DD-28. Single command,
zero config, immediately runnable. The generated structure matches
lykn's own project layout — `project.json` at root, source in
`packages/<name>/`, entry point is `mod.lykn`.

## Motivation

1. **Zero-friction start**: New users run `lykn new my-app` and
   have a working project in seconds
2. **Convention over configuration**: Every lykn project has the
   same structure — easy to navigate, easy to teach
3. **Immediately runnable**: `lykn new my-app && cd my-app && lykn run packages/my-app/mod.lykn` works out of the box
4. **Book Ch 1 needs it**: "Install lykn, create a project, run
   it" is the first chapter flow

## Usage

```sh
# Create a new project
lykn new my-app

# Create in a specific directory
lykn new my-app --path /tmp/projects

# Create with a specific template (future)
lykn new my-app --template lib
```

## Generated Structure

```
my-app/
├── project.json              ← workspace root
├── packages/
│   └── my-app/
│       ├── deno.json          ← package config
│       └── mod.lykn           ← entry point
├── test/
│   └── mod.test.js            ← starter test
└── .gitignore
```

### project.json

```json
{
    "workspace": ["./packages/my-app"],
    "imports": {
        "my-app/": "./packages/my-app/"
    },
    "tasks": {
        "test": "deno test -A test/"
    }
}
```

### packages/my-app/deno.json

```json
{
    "name": "@my-app/my-app",
    "version": "0.1.0",
    "exports": "./mod.lykn"
}
```

### packages/my-app/mod.lykn

```lisp
;; my-app — created with lykn new

(bind greeting "Hello from my-app!")
(console:log greeting)
```

### test/mod.test.js

```javascript
import { assertEquals } from "https://deno.land/std/assert/mod.ts";

Deno.test("project runs", () => {
  // Replace with real tests
  assertEquals(1 + 1, 2);
});
```

### .gitignore

```
.DS_Store
node_modules/
target/
dist/
bin/
*.js.map
```

## Implementation

### CLI Subcommand

Add `New` variant to `Commands` enum in `main.rs`:

```rust
/// Create a new lykn project
New {
    /// Project name
    name: String,
    /// Parent directory (default: current directory)
    #[arg(long)]
    path: Option<PathBuf>,
},
```

### `cmd_new` Function

```rust
fn cmd_new(name: &str, path: Option<&Path>) {
    let base = path.unwrap_or_else(|| Path::new("."));
    let project_dir = base.join(name);

    if project_dir.exists() {
        eprintln!("error: directory '{}' already exists", project_dir.display());
        process::exit(1);
    }

    // Create directory structure
    fs::create_dir_all(project_dir.join("packages").join(name))?;
    fs::create_dir_all(project_dir.join("test"))?;

    // Write files
    write_project_json(&project_dir, name);
    write_package_deno_json(&project_dir, name);
    write_mod_lykn(&project_dir, name);
    write_test(&project_dir, name);
    write_gitignore(&project_dir);

    // Init git
    Command::new("git")
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .ok();

    eprintln!("Created lykn project '{name}' in {}", project_dir.display());
    eprintln!("");
    eprintln!("  cd {name}");
    eprintln!("  lykn run packages/{name}/mod.lykn");
}
```

### File Templates

Templates are embedded as string constants in the binary — no
external template files needed. The `name` is interpolated:

- lisp-case for lykn source (`my-app`)
- kebab-case for directories (`my-app/`)
- Package name for JSON (`@my-app/my-app`)

### Name Validation

- Must be non-empty
- Must be valid kebab-case: lowercase letters, digits, hyphens
- Must not start with a digit or hyphen
- Must not be a reserved name (`test`, `packages`, `dist`, etc.)

## Templates (Future Extension)

The `--template` flag is reserved for future use:

| Template | Description |
|----------|-------------|
| `app` | Default — runnable application (current behavior) |
| `lib` | Library package (exports, no main, JSR publish config) |
| `multi` | Multi-package workspace (two packages) |
| `web` | Web application (Deno.serve, static files) |

For now, only the default `app` template exists. The `--template`
flag is not implemented in v1 — just the default behavior.

## Post-creation Message

```
Created lykn project 'my-app' in ./my-app

  cd my-app
  lykn run packages/my-app/mod.lykn    # run the project
  lykn test                             # run tests
  lykn compile packages/my-app/mod.lykn # compile to JS

Happy hacking!
```

## Edge Cases

| Case | Behavior |
|------|----------|
| Directory already exists | Error: "directory 'X' already exists" |
| Name with uppercase | Error: "project name must be kebab-case" |
| Name with spaces | Error: "project name must be kebab-case" |
| No git installed | Skip git init, no error |
| `--path` doesn't exist | Create parent directories |
| Name is `.` (current dir) | Use current directory name, don't create subdir |

## Testing

- `lykn new test-project` creates correct structure
- All generated files have correct content
- `lykn run packages/test-project/mod.lykn` works in generated project
- `lykn test` works in generated project
- Duplicate name → error
- Invalid name → error
- Cleanup: remove generated directory after test

## Documentation Updates

### README.md

Add `lykn new` to the usage section as the first step:

```markdown
### Quick Start

```sh
lykn new my-app
cd my-app
lykn run packages/my-app/mod.lykn
```

### SKILL.md

Add `lykn new` to the CLI section and the "Writing New lykn Code"
workflow:

```
## Workflows
### Writing New lykn Code
1. **Create project**: `lykn new my-project`
2. **Load anti-patterns first**: ...
```

Add to CLI quick reference:

```
lykn new my-app              # create new project
```

### CLI Guide (docs/guides/15-lykn-cli.md)

Add new section `ID-00: lykn new — Create a New Project` at the
top (before ID-01 Install), since creating a project is the first
thing a user does:

- Usage examples
- Generated structure
- Name validation rules
- `--path` option
- Post-creation message

### Project Structure Guide (docs/guides/10-project-structure.md)

Update ID-03 (canonical layout) to reference `lykn new`:

> This is the structure generated by `lykn new <name>`. All lykn
> projects follow this convention.

### CLAUDE.md

Add `lykn new` to the build commands section under a "Project
Setup" heading.

### Quick Reference Table (CLI guide)

Add row:

| `lykn new NAME` | Create new project |
