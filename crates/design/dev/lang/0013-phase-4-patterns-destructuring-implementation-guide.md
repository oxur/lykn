# Phase 4 — Patterns (Destructuring): Implementation Guide

**For**: Claude Code
**Scope**: Phase 4 of lykn v0.1.0 — destructuring patterns for object and array
**Where you're working**: `src/compiler.js` — modifying existing macros AND adding new compilation functions
**Prerequisites**: Phase 1 (camelCase, colon syntax), Phase 2 (`const`/`let`/`var`, `=`, function forms, `for-of`/`for-in`), Phase 3 (`default`, `spread`, rewritten `object`)
**Design authority**: `crates/design/dev/lang/0006-dd-06-destructuring-patterns.md`

---

## Overview: What Phase 4 Is and Why It's Different

Phase 4 is architecturally different from all previous phases. In Phases 1–3, you added independent macros to the `macros` object. In Phase 4, you're modifying how EXISTING macros work — `const`, `let`, `var`, `=`, `function`, `lambda`, `=>`, `for-of`, and `for-in` all need to detect when their children are destructuring patterns and compile them differently.

The core insight (from DD-06): **lykn uses the same forms — `object` and `array` — for both construction and destructuring.** The compiler distinguishes them by CONTEXT:

```lisp
;; Construction (expression position) → ObjectExpression / ArrayExpression
(const x (object (name "Duncan")))      ;; object is on the RIGHT of const
(const y (array 1 2 3))                 ;; array is on the RIGHT of const

;; Destructuring (pattern position) → ObjectPattern / ArrayPattern
(const (object name age) person)        ;; object is on the LEFT of const
(const (array first second) arr)        ;; array is on the LEFT of const
```

This is the **constructor-as-destructor** pattern from Erlang/ML tradition: the same syntax that builds a thing also takes it apart.

### What You're Building

| Item | Type | Notes |
|------|------|-------|
| 4.1 `compilePattern()` function | New module-level function | The heart of Phase 4 |
| 4.2 Object pattern compilation | Inside `compilePattern` | Handles `alias`, `default`, `rest`, shorthand |
| 4.3 Array pattern compilation | Inside `compilePattern` | Handles `_` skip, `default`, `rest` |
| 4.4 `rest` macro | New macro | `RestElement` for patterns |
| 4.5 Modifications to existing macros | Change existing code | `const`/`let`/`var`/`=`/function forms/for-of/for-in |

---

## The Architecture: `compilePattern()` vs `compileExpr()`

You need a new function, `compilePattern(node)`, that compiles a reader AST node as a **pattern** rather than as an **expression**. The key difference:

| What | `compileExpr(node)` produces | `compilePattern(node)` produces |
|------|-----------------------------|---------------------------------|
| `(object ...)` | `ObjectExpression` | `ObjectPattern` |
| `(array ...)` | `ArrayExpression` | `ArrayPattern` |
| atom `name` | `Identifier` (with camelCase) | `Identifier` (with camelCase) — same |
| `(default name val)` | `AssignmentPattern` | `AssignmentPattern` — same |
| `(rest x)` | `RestElement` | `RestElement` — same |
| `(alias ...)` | N/A (not a standalone macro) | `Property` with rename |

For simple atoms, `compilePattern` does the same thing as `compileExpr` — produces an `Identifier`. The difference only matters for `object` and `array` forms, where the ESTree node type changes.

### Where to Put `compilePattern`

Add it as a module-level function after `compileExpr`. It needs to call `compileExpr` for value expressions (default values, etc.) and call itself recursively for nested patterns.

### The Decision: When to Call `compilePattern` vs `compileExpr`

The calling macros (`const`, `let`, `var`, `=`, function params, `for-of`, `for-in`) decide which function to call based on whether the node is in pattern position:

```
const/let/var:  id = compilePattern(args[0])    ← PATTERN position
                init = compileExpr(args[1])      ← EXPRESSION position

=:              left = compilePattern(args[0])   ← PATTERN position  (only if it's an object/array form)
                right = compileExpr(args[1])     ← EXPRESSION position

function params: each param = compilePattern(param)  ← PATTERN position
for-of/for-in:  binding = compilePattern(args[0])    ← PATTERN position
```

---

## 4.1 The `compilePattern()` Function

### Implementation

```js
function compilePattern(node) {
  if (!node) return null;

  switch (node.type) {
    case 'atom': {
      const val = node.value;

      // _ in array patterns means skip (null element)
      if (val === '_') return null;

      // Regular identifier — same as compileExpr
      if (val === 'true' || val === 'false' || val === 'null' || val === 'undefined') {
        // These shouldn't appear in pattern position, but if they do,
        // let compileExpr handle them (they'll cause a JS error at runtime)
        return compileExpr(node);
      }

      return { type: 'Identifier', name: toCamelCase(val) };
    }

    case 'list': {
      if (node.values.length === 0) {
        // Empty list in pattern position — empty object pattern? Unlikely but handle it.
        return { type: 'ObjectPattern', properties: [] };
      }

      const head = node.values[0];
      const rest = node.values.slice(1);

      if (head.type !== 'atom') {
        // Non-atom head in pattern — not a recognized pattern form
        throw new Error('Unrecognized pattern form: expected object, array, default, rest, or alias');
      }

      switch (head.value) {
        case 'object':
          return compileObjectPattern(rest);

        case 'array':
          return compileArrayPattern(rest);

        case 'default':
          // (default name value) → AssignmentPattern
          if (rest.length !== 2) {
            throw new Error('default in pattern requires 2 arguments: (default name value)');
          }
          return {
            type: 'AssignmentPattern',
            left: compilePattern(rest[0]),
            right: compileExpr(rest[1]),
          };

        case 'rest':
          // (rest name) → RestElement
          if (rest.length !== 1) {
            throw new Error('rest requires exactly 1 argument: (rest name)');
          }
          return {
            type: 'RestElement',
            argument: compilePattern(rest[0]),
          };

        case 'alias':
          // (alias key local) or (alias key local default)
          // In pattern context, this becomes a Property node for ObjectPattern
          // But alias in isolation is not a valid top-level pattern —
          // it should only appear inside an object pattern's children.
          // If we get here, someone wrote (const (alias ...) val) at top level.
          throw new Error('alias can only appear inside an object pattern');

        default:
          // Unknown form in pattern position — probably an error,
          // but fall back to compileExpr for forward compatibility
          return compileExpr(node);
      }
    }

    default:
      // Numbers, strings in pattern position — pass through to compileExpr
      return compileExpr(node);
  }
}
```

### Compiler Pitfall: `compilePattern` Calls `compileExpr` for DEFAULT VALUES

In `(default name "world")`, the left side (`name`) is compiled as a pattern (could be nested), but the right side (`"world"`) is compiled as an expression. Don't call `compilePattern` on the default value — it's not a pattern, it's a value.

```js
// CORRECT:
left: compilePattern(rest[0]),    // pattern (the binding)
right: compileExpr(rest[1]),      // expression (the default value)

// WRONG:
left: compilePattern(rest[0]),
right: compilePattern(rest[1]),   // NO — default values are expressions
```

### Compiler Pitfall: `_` Returns `null`, Not an Identifier

The atom `_` in a pattern means "skip this element." `compilePattern` returns `null` for it. This `null` goes into `ArrayPattern.elements` to represent a skipped position: `const [, second] = arr`. Don't return an Identifier named `_` — that would create a binding called `_`.

---

## 4.2 Object Pattern Compilation

### What It Handles

```lisp
(const (object name age) person)                    ;; shorthand
(const (object (alias old-name new-name)) obj)      ;; rename
(const (object (default x 0)) obj)                  ;; default
(const (object (alias key local default-val)) obj)  ;; rename + default
(const (object (rest others)) obj)                  ;; rest
(const (object name (alias data items) (default count 0)) obj)  ;; mixed
```

### ESTree Structure: `ObjectPattern`

```js
{
  type: 'ObjectPattern',
  properties: [
    // Each is either a Property or a RestElement
  ]
}
```

Each property in an `ObjectPattern` is a `Property` node (same type as in `ObjectExpression`, but the `value` field is a `Pattern` instead of an `Expression`). REST elements are `RestElement` nodes directly in the `properties` array.

### Implementation

```js
function compileObjectPattern(children) {
  // children are the args after 'object' head
  const properties = [];

  for (let i = 0; i < children.length; i++) {
    const child = children[i];

    if (child.type === 'atom') {
      // Bare atom → shorthand binding
      // const { name } = obj
      const name = toCamelCase(child.value);
      properties.push({
        type: 'Property',
        key: { type: 'Identifier', name },
        value: { type: 'Identifier', name },
        kind: 'init',
        computed: false,
        shorthand: true,
        method: false,
      });

    } else if (child.type === 'list') {
      if (child.values.length === 0) {
        throw new Error('Empty sub-list in object pattern');
      }

      const head = child.values[0];

      // (rest others) → RestElement
      if (head.type === 'atom' && head.value === 'rest') {
        if (child.values.length !== 2) {
          throw new Error('rest requires exactly 1 argument');
        }
        if (i !== children.length - 1) {
          throw new Error('rest must be the last element in an object pattern');
        }
        properties.push({
          type: 'RestElement',
          argument: compilePattern(child.values[1]),
        });
        continue;
      }

      // (default name value) → Property with AssignmentPattern value
      if (head.type === 'atom' && head.value === 'default') {
        if (child.values.length !== 3) {
          throw new Error('default in object pattern: (default name value)');
        }
        const propName = toCamelCase(child.values[1].value);
        properties.push({
          type: 'Property',
          key: { type: 'Identifier', name: propName },
          value: {
            type: 'AssignmentPattern',
            left: { type: 'Identifier', name: propName },
            right: compileExpr(child.values[2]),
          },
          kind: 'init',
          computed: false,
          shorthand: true,    // { x = 0 } is shorthand in ESTree
          method: false,
        });
        continue;
      }

      // (alias key local) → rename
      // (alias key local default-val) → rename + default
      if (head.type === 'atom' && head.value === 'alias') {
        if (child.values.length < 3 || child.values.length > 4) {
          throw new Error('alias: (alias key local) or (alias key local default)');
        }

        const key = toCamelCase(child.values[1].value);
        let valueNode = compilePattern(child.values[2]);

        // Three-arg alias with default: (alias key local default-val)
        if (child.values.length === 4) {
          valueNode = {
            type: 'AssignmentPattern',
            left: valueNode,
            right: compileExpr(child.values[3]),
          };
        }

        properties.push({
          type: 'Property',
          key: { type: 'Identifier', name: key },
          value: valueNode,
          kind: 'init',
          computed: false,
          shorthand: false,   // NOT shorthand — key and value differ
          method: false,
        });
        continue;
      }

      // If we get here, it's an unrecognized sub-list in object pattern
      throw new Error(
        'object pattern: each element must be an atom (shorthand), ' +
        '(alias ...), (default ...), or (rest ...). Got: (' +
        (head.type === 'atom' ? head.value : head.type) + ' ...)'
      );

    } else {
      throw new Error('object pattern: unexpected ' + child.type);
    }
  }

  return { type: 'ObjectPattern', properties };
}
```

### Compiler Pitfall: `shorthand: true` for `default` Properties

This is subtle. In JS, `const { x = 0 } = obj` is a SHORTHAND property with a default. ESTree represents it as:

```js
{
  type: 'Property',
  key: Identifier('x'),
  value: AssignmentPattern(Identifier('x'), Literal(0)),
  shorthand: true   // ← despite key and value being "different" nodes
}
```

The `shorthand` flag is `true` because the source-level syntax uses shorthand — there's no explicit `:` separator. The `AssignmentPattern` wrapping the default is inside the value, but the property itself is still shorthand. If you set `shorthand: false`, astring generates `{ x: x = 0 }` instead of `{ x = 0 }`.

### Compiler Pitfall: `alias` Value Can Be a Nested Pattern

`(alias data (array first second))` — the "local" binding is itself a pattern:

```lisp
(const (object (alias data (array first second))) response)
```
→
```js
const { data: [first, second] } = response;
```

This is why `compilePattern(child.values[2])` is called (not `compileExpr`) — the value of an alias can be a destructuring pattern. If the local is a plain atom, `compilePattern` returns an `Identifier`. If it's `(array ...)` or `(object ...)`, it returns the nested pattern.

### Compiler Pitfall: `alias` Key is ALWAYS a Simple Identifier

The first argument to `alias` (the key) is always the property name being matched — it's a simple atom that becomes an `Identifier`. It does NOT go through `compilePattern`. It goes through `toCamelCase` directly. Don't call `compilePattern` on the key.

### Worked Example: Mixed Object Pattern

```lisp
(const (object name (alias data items) (default count 0) (rest extras)) response)
```

Produces:
```js
const { name, data: items, count = 0, ...extras } = response;
```

ESTree `ObjectPattern.properties`:
```
[
  Property(Identifier("name"), Identifier("name"), shorthand: true),
  Property(Identifier("data"), Identifier("items"), shorthand: false),
  Property(Identifier("count"), AssignmentPattern(Identifier("count"), Literal(0)), shorthand: true),
  RestElement(Identifier("extras"))
]
```

---

## 4.3 Array Pattern Compilation

### What It Handles

```lisp
(const (array first second) arr)             ;; basic
(const (array _ second) arr)                 ;; skip first element
(const (array _ _ third) arr)                ;; skip first two
(const (array (default x 0)) arr)            ;; default
(const (array first (rest tail)) arr)        ;; rest
(const (array (array a b) (array c d)) matrix) ;; nested
```

### ESTree Structure: `ArrayPattern`

```js
{
  type: 'ArrayPattern',
  elements: [
    Pattern | null    // null = skipped position
  ]
}
```

### Implementation

```js
function compileArrayPattern(children) {
  const elements = [];

  for (let i = 0; i < children.length; i++) {
    const child = children[i];

    if (child.type === 'atom') {
      if (child.value === '_') {
        // Skip marker → null in elements array
        elements.push(null);
      } else {
        // Regular binding
        elements.push({ type: 'Identifier', name: toCamelCase(child.value) });
      }

    } else if (child.type === 'list') {
      if (child.values.length === 0) {
        throw new Error('Empty sub-list in array pattern');
      }

      const head = child.values[0];

      // (rest name) → RestElement (must be last)
      if (head.type === 'atom' && head.value === 'rest') {
        if (child.values.length !== 2) {
          throw new Error('rest requires exactly 1 argument');
        }
        if (i !== children.length - 1) {
          throw new Error('rest must be the last element in an array pattern');
        }
        elements.push({
          type: 'RestElement',
          argument: compilePattern(child.values[1]),
        });
        continue;
      }

      // (default name value) → AssignmentPattern
      if (head.type === 'atom' && head.value === 'default') {
        if (child.values.length !== 3) {
          throw new Error('default in array pattern: (default name value)');
        }
        elements.push({
          type: 'AssignmentPattern',
          left: compilePattern(child.values[1]),
          right: compileExpr(child.values[2]),
        });
        continue;
      }

      // (object ...) or (array ...) → nested pattern
      if (head.type === 'atom' && (head.value === 'object' || head.value === 'array')) {
        elements.push(compilePattern(child));
        continue;
      }

      // Anything else — fall back to compilePattern
      elements.push(compilePattern(child));

    } else {
      // Numbers, strings — unusual in pattern position but pass through
      elements.push(compileExpr(child));
    }
  }

  return { type: 'ArrayPattern', elements };
}
```

### Compiler Pitfall: `_` Produces `null`, Not `Identifier("_")`

```lisp
(const (array _ second) pair)
```
→
```js
const [, second] = pair;
```

The `null` in `elements[0]` tells astring to emit an empty position (just a comma). If you put `Identifier("_")` instead, astring would generate `const [_, second] = pair` which creates a binding named `_` — not what the programmer intended.

### Compiler Pitfall: `rest` Position Validation

`(rest ...)` MUST be the last element in both object and array patterns. JavaScript syntax requires this — `const [head, ...tail, last]` is a syntax error. Validate the position and throw a clear error if `rest` appears anywhere but last.

### Compiler Pitfall: Nested Patterns in Array Elements

Array pattern elements can themselves be patterns:

```lisp
(const (array (array a b) (array c d)) matrix)
```
→
```js
const [[a, b], [c, d]] = matrix;
```

The `compilePattern` call on each child handles this: when it sees `(array ...)`, it recurses into `compileArrayPattern`.

---

## 4.4 `rest` Macro

Add a standalone `rest` macro so that `(rest x)` works in expression position too (e.g., rest parameters in function definitions):

```js
'rest'(args) {
  if (args.length !== 1) {
    throw new Error('rest takes exactly one argument');
  }
  return {
    type: 'RestElement',
    argument: compileExpr(args[0]),
  };
},
```

### Why Both a Macro AND Pattern Handling?

`rest` appears in two contexts:

1. **Inside a pattern** (handled by `compileObjectPattern`/`compileArrayPattern`): `(const (object (rest others)) obj)` — the pattern compilers detect `rest` structurally and build `RestElement` directly.

2. **In function parameter lists** (handled by the `rest` macro): `(function f (a b (rest args)) ...)` — the function macros compile params via `compileExpr`, which dispatches to `macros['rest']` and returns `RestElement`.

Both paths produce the same ESTree node. The pattern compilers handle `rest` inside patterns; the macro handles `rest` in other positions (primarily function params).

### Compiler Pitfall: `RestElement` vs `SpreadElement`

Repeating from Phase 3 because this is critical:
- `SpreadElement` — EXPRESSION side: `(spread x)` → `...x` in arrays, calls, objects
- `RestElement` — PATTERN side: `(rest x)` → `...x` in destructuring and function params

They produce different ESTree nodes despite identical JS output. If you use the wrong one, tools consuming the AST may break (even if astring generates correct JS).

---

## 4.5 Modifications to Existing Macros

This is the most delicate part of Phase 4. You're changing how `const`, `let`, `var`, `=`, all function forms, and `for-of`/`for-in` handle their left-hand/binding positions.

### The Principle

Every macro that has a "binding position" (a place where a pattern could appear) needs to call `compilePattern` instead of `compileExpr` for that position.

**But**: `compilePattern` handles plain atoms too (returns an `Identifier`), so changing `compileExpr(args[0])` to `compilePattern(args[0])` works for BOTH plain bindings and destructuring patterns. You don't need conditional logic — just change the call.

### 4.5a Modify `const`, `let`, `var`

**Current code** (all three are identical in structure):

```js
'const'(args) {
  return {
    type: 'VariableDeclaration',
    kind: 'const',
    declarations: [{
      type: 'VariableDeclarator',
      id: compileExpr(args[0]),      // ← CHANGE THIS
      init: args[1] ? compileExpr(args[1]) : null,
    }],
  };
},
```

**Change**: Replace `compileExpr(args[0])` with `compilePattern(args[0])`:

```js
'const'(args) {
  return {
    type: 'VariableDeclaration',
    kind: 'const',
    declarations: [{
      type: 'VariableDeclarator',
      id: compilePattern(args[0]),   // ← CHANGED: pattern position
      init: args[1] ? compileExpr(args[1]) : null,
    }],
  };
},
```

Do the same for `let` and `var`.

**Why this is safe**: When `args[0]` is a plain atom like `x`, `compilePattern` returns `Identifier("x")` — exactly what `compileExpr` returned before. When `args[0]` is `(object name age)`, `compilePattern` returns `ObjectPattern` — the new behavior. No existing code breaks.

### 4.5b Modify `=` (Assignment)

**Current code**:

```js
'='(args) {
  return {
    type: 'AssignmentExpression',
    operator: '=',
    left: compileExpr(args[0]),      // ← CHANGE THIS
    right: compileExpr(args[1]),
  };
},
```

**Change**: The left side needs `compilePattern` when it's a destructuring form, but `compileExpr` for regular assignments like `(= x 5)` or `(= this:count 0)`.

Here's the subtlety: `compilePattern` handles atoms correctly (returns `Identifier`), but it DOESN'T handle colon syntax or member expressions. `(= this:count 0)` needs `compileExpr` for the left side because `this:count` produces a `MemberExpression`, not a pattern.

The solution: detect whether the left side LOOKS like a destructuring pattern (is a list headed by `object` or `array`), and choose accordingly:

```js
'='(args) {
  if (args.length !== 2) {
    throw new Error('= requires exactly 2 arguments');
  }

  const leftNode = args[0];
  const isPattern = leftNode.type === 'list' &&
    leftNode.values.length > 0 &&
    leftNode.values[0].type === 'atom' &&
    (leftNode.values[0].value === 'object' || leftNode.values[0].value === 'array');

  return {
    type: 'AssignmentExpression',
    operator: '=',
    left: isPattern ? compilePattern(leftNode) : compileExpr(leftNode),
    right: compileExpr(args[1]),
  };
},
```

**Compiler pitfall — `=` with `MemberExpression` on the left**:

`(= this:count 0)` → `this.count = 0`. The left side is an atom `this:count`, which `compileExpr` turns into a `MemberExpression`. If you blindly called `compilePattern` here, it would try to compile `this:count` as a pattern — which would just return `Identifier("thisCount")` because `compilePattern` for atoms calls `toCamelCase` but doesn't handle colon syntax. That would be WRONG.

The pattern check above avoids this: only lists headed by `object` or `array` trigger pattern compilation.

### 4.5c Modify Function Parameter Compilation

All three function forms (`function`, `lambda`, `=>`) compile their parameter lists. Each parameter needs to be compiled as a pattern.

**Current code in `=>`**:

```js
'=>'(args) {
  const params = args[0].type === 'list'
    ? args[0].values.map(compileExpr)    // ← CHANGE THIS
    : [];
  // ... rest of arrow implementation
},
```

**Change**: Replace `compileExpr` with `compilePattern`:

```js
'=>'(args) {
  const params = args[0].type === 'list'
    ? args[0].values.map(compilePattern)  // ← CHANGED: pattern position
    : [];
  // ... rest of arrow implementation
},
```

Do the same for `lambda` (same pattern) and `function` (params are at `args[1]`):

```js
'function'(args) {
  // ...
  const params = args[1].values.map(compilePattern);  // ← CHANGED
  // ...
},
```

**Why this works**: For plain atom params like `(=> (a b) ...)`, `compilePattern` returns `Identifier` nodes — same as before. For destructured params like `(=> ((object name age)) ...)`, it returns `ObjectPattern`. For default params like `(=> ((default x 0)) ...)`, `compilePattern` dispatches to the `default` case and returns `AssignmentPattern`. For rest params like `(=> (a (rest args)) ...)`, `compilePattern` dispatches to `rest` and returns `RestElement`.

**Compiler pitfall — the `rest` macro vs `compilePattern` rest handling**:

When function params are compiled via `compilePattern`, a `(rest args)` param goes through `compilePattern`'s list handler, which detects `rest` as the head atom and returns `RestElement`. It does NOT go through `macros['rest']`. Both paths produce the same node, but in function params, `compilePattern` handles it.

If you kept `compileExpr` for params (as before), `(rest args)` would dispatch to `macros['rest']`, which also returns `RestElement`. So either approach works for `rest` in params. But you NEED `compilePattern` for destructured params like `(object name age)`, so change it.

### 4.5d Modify `for-of` and `for-in`

**Current code** (from Phase 2):

```js
'for-of'(args) {
  const binding = compileExpr(args[0]);    // ← CHANGE THIS
  // ...
},
```

**Change**:

```js
'for-of'(args) {
  const binding = compilePattern(args[0]);  // ← CHANGED: pattern position
  // ...
},
```

Same for `for-in`.

This enables:

```lisp
(for-of (array key value) (map:entries)
  (console:log key value))
```
→
```js
for (const [key, value] of map.entries()) {
  console.log(key, value);
}
```

---

## How It All Fits Together: Compilation Flow

Let's trace through a complete example:

```lisp
(const (object name (alias data items) (default count 0)) response)
```

1. `macros['const']` is called with `args = [(object name (alias data items) (default count 0)), response]`
2. `id = compilePattern(args[0])` — args[0] is a list headed by `object`
3. `compilePattern` sees a list with head atom `object`, calls `compileObjectPattern(rest)` where rest = `[name, (alias data items), (default count 0)]`
4. `compileObjectPattern` iterates:
   - `name` → atom → shorthand Property: `{ name }`
   - `(alias data items)` → alias → Property with key `data`, value `Identifier("items")`, shorthand false: `data: items`
   - `(default count 0)` → default → Property with key `count`, value `AssignmentPattern(Identifier("count"), Literal(0))`, shorthand true: `count = 0`
5. Returns `ObjectPattern` with three properties
6. `init = compileExpr(args[1])` → `Identifier("response")`
7. Result: `VariableDeclaration(const, [VariableDeclarator(ObjectPattern, Identifier("response"))])`
8. astring generates: `const { name, data: items, count = 0 } = response;`

---

## Test Cases (4.8)

### File Organization

```
test/
  forms/
    destructuring-object.test.js
    destructuring-array.test.js
    destructuring-nested.test.js
    destructuring-params.test.js
    destructuring-assignment.test.js
```

### `test/forms/destructuring-object.test.js`

```js
Deno.test("object pattern: shorthand", () => {
  const result = lykn('(const (object name age) person)');
  assertEquals(result.includes('{name, age}') || result.includes('{ name, age }'), true);
  assertEquals(result.includes('= person'), true);
});

Deno.test("object pattern: alias rename", () => {
  const result = lykn('(const (object (alias old-name new-name)) obj)');
  assertEquals(result.includes('oldName: newName'), true);
});

Deno.test("object pattern: default value", () => {
  const result = lykn('(const (object (default x 0)) point)');
  assertEquals(result.includes('x = 0'), true);
});

Deno.test("object pattern: alias with default", () => {
  const result = lykn('(const (object (alias name n "anon")) obj)');
  assertEquals(result.includes('name: n = "anon"'), true);
});

Deno.test("object pattern: rest", () => {
  const result = lykn('(const (object a (rest others)) obj)');
  assertEquals(result.includes('...others'), true);
});

Deno.test("object pattern: rest not last throws", () => {
  assertThrows(() => lykn('(const (object (rest others) a) obj)'));
});

Deno.test("object pattern: camelCase", () => {
  const result = lykn('(const (object my-name) person)');
  assertEquals(result.includes('myName'), true);
});

Deno.test("object pattern: mixed", () => {
  const result = lykn('(const (object name (alias data items) (default count 0)) resp)');
  assertEquals(result.includes('name'), true);
  assertEquals(result.includes('data: items'), true);
  assertEquals(result.includes('count = 0'), true);
});
```

### `test/forms/destructuring-array.test.js`

```js
Deno.test("array pattern: basic", () => {
  const result = lykn('(const (array first second) arr)');
  assertEquals(result.includes('[first, second]'), true);
});

Deno.test("array pattern: skip with _", () => {
  const result = lykn('(const (array _ second) pair)');
  assertEquals(result.includes('[, second]'), true);
});

Deno.test("array pattern: multiple skips", () => {
  const result = lykn('(const (array _ _ third) arr)');
  assertEquals(result.includes('[, , third]'), true);
});

Deno.test("array pattern: default", () => {
  const result = lykn('(const (array (default x 0) (default y 0)) point)');
  assertEquals(result.includes('x = 0'), true);
  assertEquals(result.includes('y = 0'), true);
});

Deno.test("array pattern: rest", () => {
  const result = lykn('(const (array head (rest tail)) list)');
  assertEquals(result.includes('...tail'), true);
});

Deno.test("array pattern: rest not last throws", () => {
  assertThrows(() => lykn('(const (array (rest head) tail) list)'));
});
```

### `test/forms/destructuring-nested.test.js`

```js
Deno.test("nested: object inside object via alias", () => {
  const result = lykn('(const (object (alias data (object name age))) response)');
  // const { data: { name, age } } = response
  assertEquals(result.includes('data:'), true);
  assertEquals(result.includes('name'), true);
});

Deno.test("nested: array inside object via alias", () => {
  const result = lykn('(const (object (alias items (array first second))) response)');
  // const { items: [first, second] } = response
  assertEquals(result.includes('items:'), true);
  assertEquals(result.includes('[first, second]'), true);
});

Deno.test("nested: array of arrays", () => {
  const result = lykn('(const (array (array a b) (array c d)) matrix)');
  // const [[a, b], [c, d]] = matrix
  assertEquals(result.includes('[['), true);
});

Deno.test("nested: deep object", () => {
  const result = lykn('(const (object (alias config (object (alias server (object host port))))) app)');
  // const { config: { server: { host, port } } } = app
  assertEquals(result.includes('host'), true);
  assertEquals(result.includes('port'), true);
});
```

### `test/forms/destructuring-params.test.js`

```js
Deno.test("params: object destructuring in arrow", () => {
  const result = lykn('(const f (=> ((object name age)) (console:log name)))');
  assertEquals(result.includes('{name, age}') || result.includes('{ name, age }'), true);
});

Deno.test("params: object destructuring in function", () => {
  const result = lykn('(function greet ((object name)) (return name))');
  assertEquals(result.includes('{name}') || result.includes('{ name }'), true);
});

Deno.test("params: mixed regular and destructured", () => {
  const result = lykn('(function handle (req (object data)) (return data))');
  assertEquals(result.includes('req'), true);
  assertEquals(result.includes('data'), true);
});

Deno.test("params: default + destructuring", () => {
  const result = lykn('(=> ((default x 0) (object name)) (+ x name))');
  assertEquals(result.includes('x = 0'), true);
  assertEquals(result.includes('name'), true);
});

Deno.test("params: rest parameter", () => {
  const result = lykn('(function f (a b (rest args)) (return args))');
  assertEquals(result.includes('...args'), true);
});

Deno.test("params: array destructuring in for-of", () => {
  const result = lykn('(for-of (array key value) entries (console:log key))');
  assertEquals(result.includes('[key, value]'), true);
});
```

### `test/forms/destructuring-assignment.test.js`

```js
Deno.test("assignment: object destructuring", () => {
  const result = lykn('(= (object a b) obj)');
  // ({a, b} = obj) or similar
  assertEquals(result.includes('a'), true);
  assertEquals(result.includes('= obj'), true);
});

Deno.test("assignment: array destructuring", () => {
  const result = lykn('(= (array x y) pair)');
  assertEquals(result.includes('[x, y]'), true);
});

Deno.test("assignment: regular (non-destructuring) still works", () => {
  assertEquals(lykn('(= x 5)'), 'x = 5;');
});

Deno.test("assignment: member expression still works", () => {
  const result = lykn('(= this:count 0)');
  assertEquals(result.includes('this.count = 0'), true);
});
```

### Important: Backward Compatibility Tests

Add tests verifying that ALL previous behavior still works after the `compilePattern` changes:

```js
Deno.test("backward compat: const with plain binding", () => {
  assertEquals(lykn('(const x 42)'), 'const x = 42;');
});

Deno.test("backward compat: let with plain binding", () => {
  assertEquals(lykn('(const my-var 42)'), 'const myVar = 42;');
});

Deno.test("backward compat: function with plain params", () => {
  const result = lykn('(function add (a b) (return (+ a b)))');
  assertEquals(result.includes('function add(a, b)'), true);
});

Deno.test("backward compat: arrow with plain params", () => {
  const result = lykn('(const f (=> (x) (* x 2)))');
  assertEquals(result.includes('(x)'), true);
});
```

---

## Summary of All Changes to `compiler.js`

| What | Where | Notes |
|------|-------|-------|
| `compilePattern()` function | Module level, after `compileExpr` | New — the heart of Phase 4 |
| `compileObjectPattern()` function | Module level, after `compilePattern` | New |
| `compileArrayPattern()` function | Module level, after above | New |
| `macros['rest']` | In `macros` object | New |
| `macros['const']` | **Modify** existing | `compileExpr(args[0])` → `compilePattern(args[0])` |
| `macros['let']` | **Modify** existing | Same change |
| `macros['var']` | **Modify** existing | Same change |
| `macros['=']` | **Modify** existing | Pattern detection for destructuring assignment |
| `macros['function']` | **Modify** existing | `compileExpr` → `compilePattern` for params |
| `macros['lambda']` | **Modify** existing | Same |
| `macros['=>']` | **Modify** existing | Same |
| `macros['for-of']` | **Modify** existing | `compileExpr` → `compilePattern` for binding |
| `macros['for-in']` | **Modify** existing | Same |

### Files Changed

| File | Action |
|------|--------|
| `src/compiler.js` | Add 3 functions + 1 macro, modify 9 existing macros |
| `test/forms/destructuring-object.test.js` | New |
| `test/forms/destructuring-array.test.js` | New |
| `test/forms/destructuring-nested.test.js` | New |
| `test/forms/destructuring-params.test.js` | New |
| `test/forms/destructuring-assignment.test.js` | New |

### What NOT to Do

- **Do not add `alias` as a standalone macro.** It's recognized structurally inside `compileObjectPattern`, not via the `macros` table.
- **Do not modify the `object` or `array` EXPRESSION macros.** `compilePattern` handles the pattern side. `macros['object']` (from Phase 3) and `macros['array']` (original) handle the expression side. They coexist: `compileExpr` dispatches to the macros, `compilePattern` dispatches to the pattern functions.
- **Do not validate that destructuring appears in a valid context.** If someone writes `(console:log (object name age))`, the `object` in call argument position goes through `compileExpr` and produces `ObjectExpression` — correct. The compiler doesn't need to "detect pattern context" globally. Each calling macro simply calls `compilePattern` for its pattern positions.
- **Do not modify the reader.** All pattern syntax uses existing reader structures (lists and atoms).

---

## Verification Checklist

- [ ] `(const (object name age) person)` → `const { name, age } = person;`
- [ ] `(const (object (alias data items)) obj)` → `const { data: items } = obj;`
- [ ] `(const (object (default x 0)) point)` → `const { x = 0 } = point;`
- [ ] `(const (object (alias name n "anon")) obj)` → `const { name: n = "anon" } = obj;`
- [ ] `(const (object (rest others)) obj)` → `const { ...others } = obj;`
- [ ] `(const (array first second) arr)` → `const [first, second] = arr;`
- [ ] `(const (array _ second) pair)` → `const [, second] = pair;`
- [ ] `(const (array head (rest tail)) list)` → `const [head, ...tail] = list;`
- [ ] `(const (object (alias data (array first second))) resp)` → `const { data: [first, second] } = resp;`
- [ ] `(= (object a b) obj)` → `({a, b} = obj)` or equivalent
- [ ] `(= x 5)` still works (plain assignment unchanged)
- [ ] `(= this:count 0)` still works (member expression assignment unchanged)
- [ ] `(=> ((object name)) name)` → arrow with destructured param
- [ ] `(function f (a (rest args)) (return args))` → rest parameter works
- [ ] `(for-of (array key value) entries (console:log key))` → destructured for-of
- [ ] ALL Phase 1, 2, 3 tests still pass (no regressions)
- [ ] `deno test test/` passes all tests
- [ ] `deno lint src/` passes
