# Lykn Design Research Prompt

## Context

Lykn is a new s-expression syntax for JavaScript. It compiles `.lykn` files
to clean, readable JS. There's a v0.0.1 PoC already published at
github.com/oxur/lykn, npmjs.com/package/lykn, jsr.io/@lykn, and crates.io.

We're now in a research/analysis phase before designing v0.1. No code changes
yet — just reading, cataloging, and producing structured analysis that will
guide design decisions.

## Repos to clone and read

Clone these into a working directory:

```sh
git clone https://github.com/estree/estree.git ./workbench/estree
git clone https://github.com/davidbonnet/astring.git ./workbench/astring
git clone https://github.com/anko/eslisp.git ./workbench/eslisp
git clone https://github.com:biwascheme/biwascheme.git ./workbench/biwascheme
git clone https://github.com/tc39/ecma262.git ./workspace/ecma262
```

## Task 1: ESTree Node Inventory

Read every `.md` file in the `estree/` repo:

- es5.md
- es2015.md (ES6)
- es2016.md
- es2017.md
- es2018.md
- es2019.md
- es2020.md
- es2021.md
- es2022.md

For each file, produce a structured list of every AST node type defined,
with its properties and which ES version introduced it.

Output format (markdown table):

```
| Node Type | ES Version | Properties | Category |
```

Where Category is one of: Expression, Statement, Declaration, Pattern,
Clause, Literal, or Other.

Save as: `research/01-estree-inventory.md`

## Task 2: Astring Coverage

Read `astring/src/astring.js` (the main source file). Find every ESTree
node type that astring knows how to generate JS for. These are typically
case labels in a switch statement or properties on a generator object.

Produce a list of supported node types. Note any that astring does NOT
support (by diffing against the ESTree inventory from Task 1).

Save as: `research/02-astring-coverage.md`

## Task 3: Eslisp Macro Table

Read eslisp's source (it's in LiveScript, `.ls` files, in `src/`). Find
the built-in macro definitions — the mapping from s-expression form names
to the ESTree AST nodes they emit.

For each built-in macro, document:

- The macro name (e.g., `var`, `if`, `lambda`, `.`, `+`)
- What ESTree node type(s) it produces
- The argument structure it expects
- Any special behavior (e.g., implicit blocks, variadic operators)

Don't worry about understanding all the LiveScript syntax perfectly —
focus on extracting the macro name → AST node mapping.

Save as: `research/03-eslisp-macros.md`

## Task 4: Gap Analysis

Cross-reference Tasks 1, 2, and 3 to produce:

### 4a: What lykn 0.0.1 handles today

Read `lykn/src/compiler.js`. List every macro/form currently implemented
and what ESTree node it produces.

### 4b: What's missing for modern JS

Compare lykn 0.0.1's coverage against the full ESTree spec (filtered to
only nodes astring supports). Produce a prioritized list of missing
forms, grouped by theme:

- **Essential** (blocking real usage): `import`/`export`, `for...of`,
  `const`/`let` destructuring patterns, template literals, spread/rest,
  arrow functions with destructured params, `async`/`await`
- **Important** (needed soon): classes, computed property names, default
  params, `for...in`, `try`/`catch`/`finally`, `throw`, `switch`,
  ternary `?:`
- **Nice to have** (can wait): generators, `for await...of`, optional
  chaining `?.`, nullish coalescing `??`, private fields, static blocks,
  logical assignment

### 4c: Proposed lykn syntax for missing forms

For each missing form, propose an s-expression syntax. Keep these design
principles in mind:

1. **Member access uses colon syntax**: `(obj:prop)` for property access,
   `(obj:method args)` for method calls. This is ZetaLisp/Common Lisp
   style. Example: `(console:log "hi")` compiles to `console.log("hi")`.

2. **Lisp-case auto-converts to camelCase**: `my-function` in lykn
   becomes `myFunction` in JS output. `my-var-name` becomes `myVarName`.
   This applies to all identifiers (atoms).

3. **No user-defined macros for now**: everything is a built-in form.
   The set of forms is fixed. Design the built-in set to be sufficient
   for writing real JS programs.

4. **No runtime**: compiled output must be plain JS with zero
   dependencies. No lykn runtime library.

5. **Thin skin over JS**: the forms should map closely to JS constructs.
   Don't invent new semantics. If JS has `for...of`, lykn should have
   a form that maps directly to it.

Note: The colon syntax `(obj:prop)` means the READER needs to be updated
to handle colons in atoms specially — splitting `console:log` into a
member expression at read time (or at compile time as syntactic sugar).
Document how you'd recommend handling this.

Save as: `research/04-gap-analysis.md`

## Task 5: Browser Shim Reference

Read BiwaScheme's source to understand how it registers a `<script>`
type handler. The relevant code is in the biwascheme repo (you can
read it from the npm package or GitHub). Look for how it:

1. Finds `<script type="text/biwascheme">` tags
2. Extracts their content
3. Evaluates/compiles it
4. Handles errors

Also look at how Wisp does the same thing (from the wisp-lang/wisp
repo, look for `<script type="application/wisp">`).

Produce a short summary of the pattern and a sketch of how lykn's
browser shim should work (compile-then-eval, not interpret).

Save as: `research/05-browser-shim.md`

## Output

All output goes in the `docs/research/` directory inside the lykn repo clone.
Each file should be well-structured markdown. Don't write any new code —
this is analysis only. Any code present should ONLY be from existing projects to
help clarify points that might otherwise be misunderstood.

## What NOT to do

- Don't modify any lykn source code
- Don't design the macro system (that's a future phase)
- Don't think about Vue, npm integration, bundlers, or frameworks
- Don't worry about TypeScript
- Don't propose a standard library
- Focus on: what ESTree nodes exist, what astring supports, what
  s-expression syntax should map to each one, and how the colon
  member-access syntax should work
