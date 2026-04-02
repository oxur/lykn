// lykn compiler
// Transforms lykn s-expression AST into ESTree nodes
// Uses astring for code generation

import { generate } from 'astring';

/** Build an ImportSpecifier from a reader node (atom or alias list). */
function buildImportSpecifier(node) {
  if (node.type === 'atom') {
    const name = toCamelCase(node.value);
    return {
      type: 'ImportSpecifier',
      imported: { type: 'Identifier', name },
      local: { type: 'Identifier', name },
    };
  }
  if (node.type === 'list' && node.values.length >= 2 &&
      node.values[0].type === 'atom' && node.values[0].value === 'alias') {
    return {
      type: 'ImportSpecifier',
      imported: { type: 'Identifier', name: toCamelCase(node.values[1].value) },
      local: { type: 'Identifier', name: toCamelCase(node.values[2].value) },
    };
  }
  throw new Error('import: each specifier must be a name or (alias original local)');
}

/** Build an ExportNamedDeclaration from a (names ...) list. */
function buildExportNames(namesNode, sourceNode) {
  const items = namesNode.values.slice(1); // skip the 'names' head
  const specifiers = items.map(item => {
    if (item.type === 'atom') {
      const name = toCamelCase(item.value);
      return {
        type: 'ExportSpecifier',
        local: { type: 'Identifier', name },
        exported: { type: 'Identifier', name },
      };
    }
    if (item.type === 'list' && item.values.length >= 2 &&
        item.values[0].type === 'atom' && item.values[0].value === 'alias') {
      return {
        type: 'ExportSpecifier',
        local: { type: 'Identifier', name: toCamelCase(item.values[1].value) },
        exported: { type: 'Identifier', name: toCamelCase(item.values[2].value) },
      };
    }
    throw new Error('export names: each item must be a name or (alias local exported)');
  });

  return {
    type: 'ExportNamedDeclaration',
    declaration: null,
    specifiers,
    source: sourceNode ? { type: 'Literal', value: sourceNode.value } : null,
  };
}

/** Convert a class member name to Identifier or PrivateIdentifier. */
function toClassKey(name) {
  const converted = toCamelCase(name);
  if (name.startsWith('-')) {
    return { type: 'PrivateIdentifier', name: converted };
  }
  return { type: 'Identifier', name: converted };
}

/** Build a TemplateElement node for template literals. */
function makeTemplateElement(raw, tail) {
  return {
    type: 'TemplateElement',
    value: { raw, cooked: raw },
    tail,
  };
}

/** Convert lisp-case to camelCase via single-pass character walk. */
function toCamelCase(str) {
  let out = '';
  let leadingHyphens = true;
  let upperNext = false;

  for (let i = 0; i < str.length; i++) {
    const ch = str[i];

    if (ch === '-') {
      if (leadingHyphens) {
        out += '_';
      } else if (i === str.length - 1) {
        out += '_';
      } else {
        upperNext = true;
      }
    } else {
      leadingHyphens = false;
      if (upperNext) {
        out += ch.toUpperCase();
        upperNext = false;
      } else {
        out += ch;
      }
    }
  }

  return out;
}

// Built-in macros: maps s-expression forms to ESTree AST nodes
const macros = {
  // Variable declaration: (var x 1)
  'var'(args) {
    const decl = {
      type: 'VariableDeclaration',
      kind: 'var',
      declarations: [{
        type: 'VariableDeclarator',
        id: compilePattern(args[0]),
        init: args[1] ? compileExpr(args[1]) : null,
      }],
    };
    return decl;
  },

  // Const declaration: (const x 1)
  'const'(args) {
    return {
      type: 'VariableDeclaration',
      kind: 'const',
      declarations: [{
        type: 'VariableDeclarator',
        id: compilePattern(args[0]),
        init: args[1] ? compileExpr(args[1]) : null,
      }],
    };
  },

  // Let declaration: (let x 1)
  'let'(args) {
    return {
      type: 'VariableDeclaration',
      kind: 'let',
      declarations: [{
        type: 'VariableDeclarator',
        id: compilePattern(args[0]),
        init: args[1] ? compileExpr(args[1]) : null,
      }],
    };
  },

  // Computed member access: (get obj key)
  'get'(args) {
    if (args.length !== 2) {
      throw new Error('get requires exactly 2 arguments: (get object key)');
    }
    return {
      type: 'MemberExpression',
      object: compileExpr(args[0]),
      property: compileExpr(args[1]),
      computed: true,
    };
  },

  // Arrow function: (=> (a b) (+ a b))
  '=>'(args) {
    const params = args[0].type === 'list'
      ? args[0].values.map(compilePattern)
      : [];
    const bodyExprs = args.slice(1);
    if (bodyExprs.length === 1) {
      const compiled = compileExpr(bodyExprs[0]);
      return {
        type: 'ArrowFunctionExpression',
        params,
        body: compiled,
        expression: true,
        async: false,
      };
    }
    return {
      type: 'ArrowFunctionExpression',
      params,
      body: {
        type: 'BlockStatement',
        body: bodyExprs.map(e => toStatement(compileExpr(e))),
      },
      expression: false,
      async: false,
    };
  },

  // Lambda: (lambda (a b) (return (+ a b)))
  'lambda'(args) {
    const params = args[0].type === 'list'
      ? args[0].values.map(compilePattern)
      : [];
    const bodyExprs = args.slice(1);
    return {
      type: 'FunctionExpression',
      id: null,
      params,
      body: {
        type: 'BlockStatement',
        body: bodyExprs.map(e => toStatement(compileExpr(e))),
      },
      async: false,
    };
  },

  // Return: (return expr)
  'return'(args) {
    return {
      type: 'ReturnStatement',
      argument: args[0] ? compileExpr(args[0]) : null,
    };
  },

  // If: (if cond then else)
  'if'(args) {
    return {
      type: 'IfStatement',
      test: compileExpr(args[0]),
      consequent: toStatement(compileExpr(args[1])),
      alternate: args[2] ? toStatement(compileExpr(args[2])) : null,
    };
  },

  // Block: (block stmt1 stmt2 ...)
  'block'(args) {
    return {
      type: 'BlockStatement',
      body: args.map(e => toStatement(compileExpr(e))),
    };
  },

  // Assignment: (= x 5) or (= (object a b) obj)
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

  // New: (new Thing arg1 arg2)
  'new'(args) {
    return {
      type: 'NewExpression',
      callee: compileExpr(args[0]),
      arguments: args.slice(1).map(compileExpr),
    };
  },

  // Array literal: (array 1 2 3)
  'array'(args) {
    return {
      type: 'ArrayExpression',
      elements: args.map(compileExpr),
    };
  },

  // Function declaration: (function name (params) body...)
  'function'(args) {
    if (args.length < 3) {
      throw new Error('function requires a name, params list, and body: (function name (params) body...)');
    }
    if (args[0].type !== 'atom') {
      throw new Error('function name must be an identifier, not a ' + args[0].type);
    }
    if (args[1].type !== 'list') {
      throw new Error('function params must be a list: (function name (params) body...)');
    }
    const params = args[1].values.map(compilePattern);
    const bodyExprs = args.slice(2);
    return {
      type: 'FunctionDeclaration',
      id: { type: 'Identifier', name: toCamelCase(args[0].value) },
      params,
      body: {
        type: 'BlockStatement',
        body: bodyExprs.map(e => toStatement(compileExpr(e))),
      },
      async: false,
      generator: false,
    };
  },

  // Async wrapper: (async (function/lambda/=> ...))
  'async'(args) {
    if (args.length !== 1) {
      throw new Error('async takes exactly one argument: (async (function/lambda/=> ...))');
    }
    const child = args[0];
    if (child.type !== 'list' || child.values.length === 0) {
      throw new Error('async argument must be a function form: (async (function/lambda/=> ...))');
    }
    const head = child.values[0];
    if (head.type !== 'atom' || !['function', 'lambda', '=>'].includes(head.value)) {
      throw new Error(
        'async can only wrap function, lambda, or =>: got ' +
        (head.type === 'atom' ? head.value : head.type)
      );
    }
    const compiled = compileExpr(child);
    compiled.async = true;
    return compiled;
  },

  // Await expression: (await expr)
  'await'(args) {
    if (args.length !== 1) {
      throw new Error('await takes exactly one argument');
    }
    return {
      type: 'AwaitExpression',
      argument: compileExpr(args[0]),
    };
  },

  // Import: (import "mod" ...) — various forms
  'import'(args) {
    if (args.length === 0) {
      throw new Error('import requires at least a module path');
    }
    if (args[0].type !== 'string') {
      throw new Error('import: first argument must be a module path string');
    }
    const source = { type: 'Literal', value: args[0].value };
    const specifiers = [];

    if (args.length === 1) {
      // (import "mod") → side-effect import
    } else if (args.length === 2) {
      if (args[1].type === 'atom') {
        // (import "mod" name) → default import
        specifiers.push({
          type: 'ImportDefaultSpecifier',
          local: { type: 'Identifier', name: toCamelCase(args[1].value) },
        });
      } else if (args[1].type === 'list') {
        // (import "mod" (a b)) → named imports
        for (const spec of args[1].values) {
          specifiers.push(buildImportSpecifier(spec));
        }
      } else {
        throw new Error('import: second argument must be a name or list of names');
      }
    } else if (args.length === 3) {
      // (import "mod" name (a b)) → default + named
      if (args[1].type !== 'atom') {
        throw new Error('import: default import name must be an identifier');
      }
      if (args[2].type !== 'list') {
        throw new Error('import: named imports must be a list');
      }
      specifiers.push({
        type: 'ImportDefaultSpecifier',
        local: { type: 'Identifier', name: toCamelCase(args[1].value) },
      });
      for (const spec of args[2].values) {
        specifiers.push(buildImportSpecifier(spec));
      }
    } else {
      throw new Error('import: too many arguments');
    }

    return {
      type: 'ImportDeclaration',
      specifiers,
      source,
    };
  },

  // Export: (export ...) — various forms
  'export'(args) {
    if (args.length === 0) {
      throw new Error('export requires an argument');
    }

    // Case 1: (export default expr)
    if (args[0].type === 'atom' && args[0].value === 'default') {
      if (args.length !== 2) {
        throw new Error('export default takes exactly one expression');
      }
      return {
        type: 'ExportDefaultDeclaration',
        declaration: compileExpr(args[1]),
      };
    }

    // Case 2: (export "mod" (names ...)) → re-export
    if (args[0].type === 'string') {
      if (args.length !== 2 || args[1].type !== 'list') {
        throw new Error('export re-export: (export "mod" (names ...))');
      }
      return buildExportNames(args[1], args[0]);
    }

    // Case 3: (export (names ...)) → export existing bindings
    if (args[0].type === 'list' && args[0].values.length > 0 &&
        args[0].values[0].type === 'atom' && args[0].values[0].value === 'names') {
      return buildExportNames(args[0], null);
    }

    // Case 4: (export (const/let/var/function ...)) → export declaration
    if (args.length === 1) {
      const decl = compileExpr(args[0]);
      return {
        type: 'ExportNamedDeclaration',
        declaration: decl,
        specifiers: [],
        source: null,
      };
    }

    throw new Error('export: unrecognized form');
  },

  // Dynamic import expression: (dynamic-import expr)
  'dynamic-import'(args) {
    if (args.length !== 1) {
      throw new Error('dynamic-import takes exactly one argument');
    }
    return {
      type: 'ImportExpression',
      source: compileExpr(args[0]),
    };
  },

  // Throw: (throw expr)
  'throw'(args) {
    if (args.length !== 1) {
      throw new Error('throw takes exactly one argument');
    }
    return {
      type: 'ThrowStatement',
      argument: compileExpr(args[0]),
    };
  },

  // Try/catch/finally: (try body... (catch e body...) (finally body...))
  'try'(args) {
    if (args.length === 0) {
      throw new Error('try requires a body');
    }

    let handler = null;
    let finalizer = null;
    let bodyEnd = args.length;

    // Check last arg for finally
    const lastArg = args[args.length - 1];
    if (lastArg.type === 'list' && lastArg.values.length > 0 &&
        lastArg.values[0].type === 'atom' && lastArg.values[0].value === 'finally') {
      finalizer = {
        type: 'BlockStatement',
        body: lastArg.values.slice(1).map(e => toStatement(compileExpr(e))),
      };
      bodyEnd--;
    }

    // Check the (possibly new) last arg for catch
    if (bodyEnd > 0) {
      const catchArg = args[bodyEnd - 1];
      if (catchArg.type === 'list' && catchArg.values.length > 0 &&
          catchArg.values[0].type === 'atom' && catchArg.values[0].value === 'catch') {
        const catchParam = catchArg.values[1];
        handler = {
          type: 'CatchClause',
          param: compileExpr(catchParam),
          body: {
            type: 'BlockStatement',
            body: catchArg.values.slice(2).map(e => toStatement(compileExpr(e))),
          },
        };
        bodyEnd--;
      }
    }

    if (!handler && !finalizer) {
      throw new Error('try requires at least a catch or finally clause');
    }

    return {
      type: 'TryStatement',
      block: {
        type: 'BlockStatement',
        body: args.slice(0, bodyEnd).map(e => toStatement(compileExpr(e))),
      },
      handler,
      finalizer,
    };
  },

  // While: (while test body...)
  'while'(args) {
    if (args.length < 2) {
      throw new Error('while requires a test and body');
    }
    return {
      type: 'WhileStatement',
      test: compileExpr(args[0]),
      body: {
        type: 'BlockStatement',
        body: args.slice(1).map(e => toStatement(compileExpr(e))),
      },
    };
  },

  // Do-while: (do-while test body...) — test first for consistency with while
  'do-while'(args) {
    if (args.length < 2) {
      throw new Error('do-while requires a test and body');
    }
    return {
      type: 'DoWhileStatement',
      test: compileExpr(args[0]),
      body: {
        type: 'BlockStatement',
        body: args.slice(1).map(e => toStatement(compileExpr(e))),
      },
    };
  },

  // C-style for: (for init test update body...)
  'for'(args) {
    if (args.length < 4) {
      throw new Error('for requires init, test, update, and body: (for init test update body...)');
    }
    const init = args[0].type === 'list' && args[0].values.length === 0
      ? null
      : compileExpr(args[0]);
    const test = args[1].type === 'list' && args[1].values.length === 0
      ? null
      : compileExpr(args[1]);
    const update = args[2].type === 'list' && args[2].values.length === 0
      ? null
      : compileExpr(args[2]);
    return {
      type: 'ForStatement',
      init,
      test,
      update,
      body: {
        type: 'BlockStatement',
        body: args.slice(3).map(e => toStatement(compileExpr(e))),
      },
    };
  },

  // For-of: (for-of binding iterable body...)
  'for-of'(args) {
    if (args.length < 3) {
      throw new Error('for-of requires binding, iterable, and body');
    }
    const binding = compilePattern(args[0]);
    return {
      type: 'ForOfStatement',
      left: {
        type: 'VariableDeclaration',
        kind: 'const',
        declarations: [{
          type: 'VariableDeclarator',
          id: binding,
          init: null,
        }],
      },
      right: compileExpr(args[1]),
      body: {
        type: 'BlockStatement',
        body: args.slice(2).map(e => toStatement(compileExpr(e))),
      },
      await: false,
    };
  },

  // For-in: (for-in binding object body...)
  'for-in'(args) {
    if (args.length < 3) {
      throw new Error('for-in requires binding, object, and body');
    }
    const binding = compilePattern(args[0]);
    return {
      type: 'ForInStatement',
      left: {
        type: 'VariableDeclaration',
        kind: 'const',
        declarations: [{
          type: 'VariableDeclarator',
          id: binding,
          init: null,
        }],
      },
      right: compileExpr(args[1]),
      body: {
        type: 'BlockStatement',
        body: args.slice(2).map(e => toStatement(compileExpr(e))),
      },
    };
  },

  // Break: (break) or (break label)
  'break'(args) {
    return {
      type: 'BreakStatement',
      label: args.length > 0
        ? { type: 'Identifier', name: toCamelCase(args[0].value) }
        : null,
    };
  },

  // Continue: (continue) or (continue label)
  'continue'(args) {
    return {
      type: 'ContinueStatement',
      label: args.length > 0
        ? { type: 'Identifier', name: toCamelCase(args[0].value) }
        : null,
    };
  },

  // Switch: (switch expr (test body...) ... (default body...))
  'switch'(args) {
    if (args.length < 2) {
      throw new Error('switch requires a discriminant and at least one case');
    }
    const discriminant = compileExpr(args[0]);
    const cases = args.slice(1).map(caseNode => {
      if (caseNode.type !== 'list' || caseNode.values.length === 0) {
        throw new Error('switch: each case must be a list (test body...)');
      }
      const headNode = caseNode.values[0];
      const isDefault = headNode.type === 'atom' && headNode.value === 'default';
      const test = isDefault ? null : compileExpr(headNode);
      const consequent = caseNode.values.slice(1)
        .map(e => toStatement(compileExpr(e)));
      return {
        type: 'SwitchCase',
        test,
        consequent,
      };
    });
    return {
      type: 'SwitchStatement',
      discriminant,
      cases,
    };
  },

  // Ternary: (? test consequent alternate)
  '?'(args) {
    if (args.length !== 3) {
      throw new Error('? (ternary) requires exactly 3 arguments: (? test then else)');
    }
    return {
      type: 'ConditionalExpression',
      test: compileExpr(args[0]),
      consequent: compileExpr(args[1]),
      alternate: compileExpr(args[2]),
    };
  },

  // Prefix increment: (++ x)
  '++'(args) {
    if (args.length !== 1) {
      throw new Error('++ takes exactly one argument');
    }
    return {
      type: 'UpdateExpression',
      operator: '++',
      argument: compileExpr(args[0]),
      prefix: true,
    };
  },

  // Prefix decrement: (-- x)
  '--'(args) {
    if (args.length !== 1) {
      throw new Error('-- takes exactly one argument');
    }
    return {
      type: 'UpdateExpression',
      operator: '--',
      argument: compileExpr(args[0]),
      prefix: true,
    };
  },

  // Label: (label name body)
  'label'(args) {
    if (args.length !== 2) {
      throw new Error('label requires a name and body: (label name body)');
    }
    return {
      type: 'LabeledStatement',
      label: { type: 'Identifier', name: toCamelCase(args[0].value) },
      body: toStatement(compileExpr(args[1])),
    };
  },

  // Debugger: (debugger)
  'debugger'(args) {
    if (args.length !== 0) {
      throw new Error('debugger takes no arguments');
    }
    return {
      type: 'DebuggerStatement',
    };
  },

  // Sequence expression: (seq expr1 expr2 ...)
  'seq'(args) {
    if (args.length < 2) {
      throw new Error('seq requires at least 2 expressions');
    }
    return {
      type: 'SequenceExpression',
      expressions: args.map(compileExpr),
    };
  },

  // Regex literal: (regex pattern) or (regex pattern flags)
  'regex'(args) {
    if (args.length < 1 || args.length > 2) {
      throw new Error('regex takes 1 or 2 arguments: (regex pattern) or (regex pattern flags)');
    }
    if (args[0].type !== 'string') {
      throw new Error('regex pattern must be a string');
    }
    const pattern = args[0].value;
    const flags = args.length === 2
      ? (args[1].type === 'string' ? args[1].value : String(args[1].value))
      : '';
    return {
      type: 'Literal',
      value: null,
      regex: { pattern, flags },
    };
  },

  // Template literal: (template "Hello, " name "!")
  'template'(args) {
    if (args.length === 0) {
      return {
        type: 'TemplateLiteral',
        quasis: [makeTemplateElement('', true)],
        expressions: [],
      };
    }

    const quasis = [];
    const expressions = [];
    let currentSegment = '';

    for (let i = 0; i < args.length; i++) {
      if (args[i].type === 'string') {
        currentSegment += args[i].value;
      } else {
        quasis.push(makeTemplateElement(currentSegment, false));
        currentSegment = '';
        expressions.push(compileExpr(args[i]));
      }
    }

    quasis.push(makeTemplateElement(currentSegment, true));

    return {
      type: 'TemplateLiteral',
      quasis,
      expressions,
    };
  },

  // Tagged template literal: (tag fn (template ...))
  'tag'(args) {
    if (args.length !== 2) {
      throw new Error('tag requires exactly 2 arguments: (tag function (template ...))');
    }
    if (args[1].type !== 'list' || args[1].values.length === 0 ||
        args[1].values[0].type !== 'atom' || args[1].values[0].value !== 'template') {
      throw new Error('tag: second argument must be a (template ...) form');
    }
    const tag = compileExpr(args[0]);
    const quasi = compileExpr(args[1]);
    return {
      type: 'TaggedTemplateExpression',
      tag,
      quasi,
    };
  },

  // Spread element: (spread expr)
  'spread'(args) {
    if (args.length !== 1) {
      throw new Error('spread takes exactly one argument');
    }
    return {
      type: 'SpreadElement',
      argument: compileExpr(args[0]),
    };
  },

  // Default parameter value: (default name value)
  'default'(args) {
    if (args.length !== 2) {
      throw new Error('default requires exactly 2 arguments: (default name value)');
    }
    return {
      type: 'AssignmentPattern',
      left: compileExpr(args[0]),
      right: compileExpr(args[1]),
    };
  },

  // Class declaration: (class Name (Super) body...)
  'class'(args) {
    if (args.length < 2) {
      throw new Error('class requires a name and superclass list: (class Name (Super) body...)');
    }
    if (args[0].type !== 'atom') {
      throw new Error('class name must be an identifier');
    }
    if (args[1].type !== 'list') {
      throw new Error('class superclass must be a list: () for no extends, (Super) for extends');
    }
    const name = { type: 'Identifier', name: toCamelCase(args[0].value) };
    const superClass = args[1].values.length > 0
      ? compileExpr(args[1].values[0])
      : null;
    return {
      type: 'ClassDeclaration',
      id: name,
      superClass,
      body: {
        type: 'ClassBody',
        body: compileClassBody(args.slice(2)),
      },
    };
  },

  // Class expression: (class-expr (Super) body...)
  'class-expr'(args) {
    if (args.length < 1) {
      throw new Error('class-expr requires a superclass list: (class-expr (Super) body...)');
    }
    if (args[0].type !== 'list') {
      throw new Error('class-expr superclass must be a list');
    }
    const superClass = args[0].values.length > 0
      ? compileExpr(args[0].values[0])
      : null;
    return {
      type: 'ClassExpression',
      id: null,
      superClass,
      body: {
        type: 'ClassBody',
        body: compileClassBody(args.slice(1)),
      },
    };
  },

  // Rest element: (rest x) — for function params
  'rest'(args) {
    if (args.length !== 1) {
      throw new Error('rest takes exactly one argument');
    }
    return {
      type: 'RestElement',
      argument: compileExpr(args[0]),
    };
  },

  // Object literal: (object (name "Duncan") (age 42)) — grouped pairs
  'object'(args) {
    const properties = [];

    for (const child of args) {
      if (child.type === 'atom') {
        // Bare atom → shorthand property
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
          throw new Error('object: empty sub-list is not allowed');
        }

        // Check for (spread expr)
        if (child.values[0].type === 'atom' && child.values[0].value === 'spread') {
          if (child.values.length !== 2) {
            throw new Error('spread takes exactly one argument');
          }
          properties.push({
            type: 'SpreadElement',
            argument: compileExpr(child.values[1]),
          });
          continue;
        }

        // Check for ((computed key-expr) value)
        if (child.values[0].type === 'list') {
          const innerList = child.values[0];
          if (innerList.values.length === 2 &&
              innerList.values[0].type === 'atom' &&
              innerList.values[0].value === 'computed') {
            if (child.values.length !== 2) {
              throw new Error('object: computed property requires a value: ((computed key) value)');
            }
            properties.push({
              type: 'Property',
              key: compileExpr(innerList.values[1]),
              value: compileExpr(child.values[1]),
              kind: 'init',
              computed: true,
              shorthand: false,
              method: false,
            });
            continue;
          }
        }

        // Single-element sub-list → error
        if (child.values.length === 1) {
          throw new Error(
            'object: single-element sub-list (' +
            (child.values[0].type === 'atom' ? child.values[0].value : '...') +
            ') is ambiguous — use a bare atom for shorthand'
          );
        }

        if (child.values.length !== 2) {
          throw new Error('object: each property must be (key value), got ' + child.values.length + ' elements');
        }

        // Regular (key value) pair
        const keyNode = child.values[0];
        properties.push({
          type: 'Property',
          key: keyNode.type === 'atom'
            ? { type: 'Identifier', name: toCamelCase(keyNode.value) }
            : compileExpr(keyNode),
          value: compileExpr(child.values[1]),
          kind: 'init',
          computed: false,
          shorthand: false,
          method: false,
        });
      } else {
        throw new Error('object: each element must be an atom (shorthand) or a list (key value)');
      }
    }

    return { type: 'ObjectExpression', properties };
  },
};

// Binary/logical operators
const binaryOps = ['+', '-', '*', '/', '%', '**', '===', '!==', '==', '!=',
                    '<', '>', '<=', '>=', '&&', '||', '??',
                    '&', '|', '^', '<<', '>>', '>>>',
                    'in', 'instanceof'];
for (const op of binaryOps) {
  macros[op] = (args) => {
    const type = (op === '&&' || op === '||' || op === '??')
      ? 'LogicalExpression'
      : 'BinaryExpression';
    let result = {
      type,
      operator: op,
      left: compileExpr(args[0]),
      right: compileExpr(args[1]),
    };
    // Support n-ary: (+ a b c) => a + b + c
    for (let i = 2; i < args.length; i++) {
      result = { type, operator: op, left: result, right: compileExpr(args[i]) };
    }
    return result;
  };
}

// Unary prefix: (! x), (typeof x)
for (const op of ['!', '~', 'typeof', 'void', 'delete']) {
  macros[op] = (args) => ({
    type: 'UnaryExpression',
    operator: op,
    prefix: true,
    argument: compileExpr(args[0]),
  });
}

// Compound assignment operators
const compoundAssignOps = [
  '+=', '-=', '*=', '/=', '%=', '**=',
  '<<=', '>>=', '>>>=',
  '&=', '|=', '^=',
  '&&=', '||=', '??=',
];
for (const op of compoundAssignOps) {
  macros[op] = (args) => {
    if (args.length !== 2) {
      throw new Error(op + ' takes exactly 2 arguments');
    }
    return {
      type: 'AssignmentExpression',
      operator: op,
      left: compileExpr(args[0]),
      right: compileExpr(args[1]),
    };
  };
}

// Ensure a node is wrapped as a statement
function toStatement(node) {
  if (!node) return { type: 'EmptyStatement' };
  if (node.type.endsWith('Statement') || node.type.endsWith('Declaration')) {
    return node;
  }
  return { type: 'ExpressionStatement', expression: node };
}

// Compile a single s-expression node to an ESTree node
export function compileExpr(node) {
  if (!node) return { type: 'Literal', value: null };

  switch (node.type) {
    case 'number':
      return { type: 'Literal', value: node.value };
    case 'string':
      return { type: 'Literal', value: node.value };
    case 'keyword':
      return { type: 'Literal', value: toCamelCase(node.value) };
    case 'atom': {
      const val = node.value;

      // 1. Literal atoms
      if (val === 'true') return { type: 'Literal', value: true };
      if (val === 'false') return { type: 'Literal', value: false };
      if (val === 'null') return { type: 'Literal', value: null };
      if (val === 'undefined') return { type: 'Identifier', name: 'undefined' };

      // 2. Special keyword atoms
      if (val === 'this') return { type: 'ThisExpression' };
      if (val === 'super') return { type: 'Super' };

      // 3. Colon syntax → MemberExpression chain
      if (val.includes(':')) {
        if (val === ':') {
          throw new Error('Bare colon is not a valid identifier');
        }
        if (val.endsWith(':')) {
          throw new Error('Trailing colon in member expression');
        }

        const segments = val.split(':');
        for (const seg of segments) {
          if (seg === '') {
            throw new Error('Empty segment in colon syntax (consecutive colons)');
          }
          if (/^\d/.test(seg)) {
            throw new Error(
              `Numeric segment "${seg}" in colon syntax — use (get obj ${seg}) for computed access`
            );
          }
        }

        const first = segments[0];
        let result;
        if (first === 'this') {
          result = { type: 'ThisExpression' };
        } else if (first === 'super') {
          result = { type: 'Super' };
        } else {
          result = { type: 'Identifier', name: toCamelCase(first) };
        }

        for (let i = 1; i < segments.length; i++) {
          const seg = segments[i];
          const isPrivate = seg.startsWith('-');
          const propName = toCamelCase(seg);
          result = {
            type: 'MemberExpression',
            object: result,
            property: isPrivate
              ? { type: 'PrivateIdentifier', name: propName }
              : { type: 'Identifier', name: propName },
            computed: false,
          };
        }

        return result;
      }

      // 4. Regular identifier with camelCase
      return { type: 'Identifier', name: toCamelCase(val) };
    }
    case 'list': {
      if (node.values.length === 0) {
        return { type: 'ArrayExpression', elements: [] };
      }
      const head = node.values[0];
      const rest = node.values.slice(1);

      // Check if head matches a macro
      if (head.type === 'atom' && macros[head.value]) {
        return macros[head.value](rest);
      }

      // Otherwise it's a function call
      return {
        type: 'CallExpression',
        callee: compileExpr(head),
        arguments: rest.map(compileExpr),
        optional: false,
      };
    }
    default:
      throw new Error(`Unknown node type: ${node.type}`);
  }
}

// Compile a reader AST node as a destructuring pattern
function compilePattern(node) {
  if (!node) return null;

  switch (node.type) {
    case 'atom': {
      const val = node.value;
      if (val === '_') return null;
      if (val === 'true' || val === 'false' || val === 'null' || val === 'undefined') {
        return compileExpr(node);
      }
      return { type: 'Identifier', name: toCamelCase(val) };
    }

    case 'list': {
      if (node.values.length === 0) {
        return { type: 'ObjectPattern', properties: [] };
      }

      const head = node.values[0];
      const rest = node.values.slice(1);

      if (head.type !== 'atom') {
        throw new Error('Unrecognized pattern form: expected object, array, default, rest, or alias');
      }

      switch (head.value) {
        case 'object':
          return compileObjectPattern(rest);

        case 'array':
          return compileArrayPattern(rest);

        case 'default':
          if (rest.length !== 2) {
            throw new Error('default in pattern requires 2 arguments: (default name value)');
          }
          return {
            type: 'AssignmentPattern',
            left: compilePattern(rest[0]),
            right: compileExpr(rest[1]),
          };

        case 'rest':
          if (rest.length !== 1) {
            throw new Error('rest requires exactly 1 argument: (rest name)');
          }
          return {
            type: 'RestElement',
            argument: compilePattern(rest[0]),
          };

        case 'alias':
          throw new Error('alias can only appear inside an object pattern');

        default:
          return compileExpr(node);
      }
    }

    default:
      return compileExpr(node);
  }
}

// Compile children of (object ...) in pattern position → ObjectPattern
function compileObjectPattern(children) {
  const properties = [];

  for (let i = 0; i < children.length; i++) {
    const child = children[i];

    if (child.type === 'atom') {
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
          shorthand: true,
          method: false,
        });
        continue;
      }

      // (alias key local) or (alias key local default-val)
      if (head.type === 'atom' && head.value === 'alias') {
        if (child.values.length < 3 || child.values.length > 4) {
          throw new Error('alias: (alias key local) or (alias key local default)');
        }

        const key = toCamelCase(child.values[1].value);
        let valueNode = compilePattern(child.values[2]);

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
          shorthand: false,
          method: false,
        });
        continue;
      }

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

// Compile children of (array ...) in pattern position → ArrayPattern
function compileArrayPattern(children) {
  const elements = [];

  for (let i = 0; i < children.length; i++) {
    const child = children[i];

    if (child.type === 'atom') {
      if (child.value === '_') {
        elements.push(null);
      } else {
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

      // Nested pattern or other form
      elements.push(compilePattern(child));

    } else {
      elements.push(compileExpr(child));
    }
  }

  return { type: 'ArrayPattern', elements };
}

// Compile class body elements into MethodDefinition/PropertyDefinition nodes
function compileClassBody(elements) {
  return elements.map(el => compileClassMember(el, false));
}

function compileClassMember(node, isStatic) {
  if (node.type !== 'list' || node.values.length === 0) {
    throw new Error('Class body element must be a non-empty list');
  }

  const head = node.values[0];
  if (head.type !== 'atom') {
    throw new Error('Class body element must start with an atom');
  }

  const headVal = head.value;

  // static wrapper: (static (...))
  if (headVal === 'static') {
    if (node.values.length !== 2) {
      throw new Error('static wraps exactly one class member: (static (member ...))');
    }
    return compileClassMember(node.values[1], true);
  }

  // async wrapper: (async (method-name (params) body...))
  if (headVal === 'async') {
    if (node.values.length !== 2) {
      throw new Error('async in class body wraps exactly one method');
    }
    const inner = node.values[1];
    if (inner.type !== 'list' || inner.values.length === 0) {
      throw new Error('async must wrap a method definition');
    }
    const innerHead = inner.values[0];
    if (innerHead.type === 'atom' && (innerHead.value === 'get' || innerHead.value === 'set')) {
      const member = compileAccessorMethod(inner, innerHead.value, isStatic);
      member.value.async = true;
      return member;
    }
    const member = compileMethod(inner, isStatic);
    member.value.async = true;
    return member;
  }

  // field: (field name) or (field name value)
  if (headVal === 'field') {
    if (node.values.length < 2 || node.values.length > 3) {
      throw new Error('field: (field name) or (field name value)');
    }
    const fieldName = node.values[1].value;
    const key = toClassKey(fieldName);
    const value = node.values.length === 3 ? compileExpr(node.values[2]) : null;
    return {
      type: 'PropertyDefinition',
      key,
      value,
      computed: false,
      static: isStatic,
    };
  }

  // get/set accessor: (get name (params) body...) or (set name (params) body...)
  if (headVal === 'get' || headVal === 'set') {
    return compileAccessorMethod(node, headVal, isStatic);
  }

  // Regular method (or constructor)
  return compileMethod(node, isStatic);
}

function compileMethod(node, isStatic) {
  if (node.values.length < 3) {
    throw new Error('Method requires name, params, and body: (name (params) body...)');
  }
  const nameAtom = node.values[0];
  if (nameAtom.type !== 'atom') {
    throw new Error('Method name must be an atom');
  }
  const methodName = nameAtom.value;
  const key = toClassKey(methodName);
  const isConstructor = methodName === 'constructor';

  const paramsList = node.values[1];
  if (paramsList.type !== 'list') {
    throw new Error('Method params must be a list');
  }
  const params = paramsList.values.map(compilePattern);
  const bodyExprs = node.values.slice(2);

  return {
    type: 'MethodDefinition',
    key,
    value: {
      type: 'FunctionExpression',
      id: null,
      params,
      body: {
        type: 'BlockStatement',
        body: bodyExprs.map(e => toStatement(compileExpr(e))),
      },
      async: false,
      generator: false,
    },
    kind: isConstructor ? 'constructor' : 'method',
    computed: false,
    static: isStatic,
  };
}

function compileAccessorMethod(node, accessorKind, isStatic) {
  if (node.values.length < 4) {
    throw new Error(accessorKind + ' accessor: (' + accessorKind + ' name (params) body...)');
  }
  const nameAtom = node.values[1];
  if (nameAtom.type !== 'atom') {
    throw new Error('Accessor name must be an atom');
  }
  const key = toClassKey(nameAtom.value);

  const paramsList = node.values[2];
  if (paramsList.type !== 'list') {
    throw new Error('Accessor params must be a list');
  }
  const params = paramsList.values.map(compilePattern);
  const bodyExprs = node.values.slice(3);

  return {
    type: 'MethodDefinition',
    key,
    value: {
      type: 'FunctionExpression',
      id: null,
      params,
      body: {
        type: 'BlockStatement',
        body: bodyExprs.map(e => toStatement(compileExpr(e))),
      },
      async: false,
      generator: false,
    },
    kind: accessorKind,
    computed: false,
    static: isStatic,
  };
}

// Compile an array of top-level s-expressions to a JS program string
export function compile(exprs) {
  const program = {
    type: 'Program',
    body: exprs.map(e => toStatement(compileExpr(e))),
    sourceType: 'module',
  };
  return generate(program, { indent: '  ' });
}
