// lykn compiler
// Transforms lykn s-expression AST into ESTree nodes
// Uses astring for code generation

import { generate } from 'astring';

// Built-in macros: maps s-expression forms to ESTree AST nodes
const macros = {
  // Variable declaration: (var x 1)
  'var'(args) {
    const decl = {
      type: 'VariableDeclaration',
      kind: 'var',
      declarations: [{
        type: 'VariableDeclarator',
        id: compileExpr(args[0]),
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
        id: compileExpr(args[0]),
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
        id: compileExpr(args[0]),
        init: args[1] ? compileExpr(args[1]) : null,
      }],
    };
  },

  // Property access: (. obj prop1 prop2)
  '.'(args) {
    let result = compileExpr(args[0]);
    for (let i = 1; i < args.length; i++) {
      const prop = args[i];
      if (prop.type === 'atom') {
        result = {
          type: 'MemberExpression',
          object: result,
          property: { type: 'Identifier', name: prop.value },
          computed: false,
        };
      } else if (prop.type === 'number') {
        result = {
          type: 'MemberExpression',
          object: result,
          property: { type: 'Literal', value: prop.value },
          computed: true,
        };
      } else if (prop.type === 'string') {
        result = {
          type: 'MemberExpression',
          object: result,
          property: { type: 'Literal', value: prop.value },
          computed: true,
        };
      } else {
        result = {
          type: 'MemberExpression',
          object: result,
          property: compileExpr(prop),
          computed: true,
        };
      }
    }
    return result;
  },

  // Arrow function: (=> (a b) (+ a b))
  '=>'(args) {
    const params = args[0].type === 'list'
      ? args[0].values.map(compileExpr)
      : [];
    const bodyExprs = args.slice(1);
    if (bodyExprs.length === 1) {
      const compiled = compileExpr(bodyExprs[0]);
      return {
        type: 'ArrowFunctionExpression',
        params,
        body: compiled,
        expression: true,
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
    };
  },

  // Lambda: (lambda (a b) (return (+ a b)))
  'lambda'(args) {
    const params = args[0].type === 'list'
      ? args[0].values.map(compileExpr)
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

  // Assignment: (= x 5)
  '='(args) {
    return {
      type: 'AssignmentExpression',
      operator: '=',
      left: compileExpr(args[0]),
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

  // Object literal: (object key1 val1 key2 val2)
  'object'(args) {
    const properties = [];
    for (let i = 0; i < args.length; i += 2) {
      properties.push({
        type: 'Property',
        key: args[i].type === 'atom'
          ? { type: 'Identifier', name: args[i].value }
          : compileExpr(args[i]),
        value: compileExpr(args[i + 1]),
        kind: 'init',
        computed: false,
        shorthand: false,
        method: false,
      });
    }
    return { type: 'ObjectExpression', properties };
  },
};

// Binary/logical operators
const binaryOps = ['+', '-', '*', '/', '%', '===', '!==', '==', '!=',
                    '<', '>', '<=', '>=', '&&', '||', '??',
                    '&', '|', '^', '<<', '>>', '>>>'];
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
    case 'atom':
      if (node.value === 'true') return { type: 'Literal', value: true };
      if (node.value === 'false') return { type: 'Literal', value: false };
      if (node.value === 'null') return { type: 'Literal', value: null };
      if (node.value === 'undefined') return { type: 'Identifier', name: 'undefined' };
      return { type: 'Identifier', name: node.value };
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

// Compile an array of top-level s-expressions to a JS program string
export function compile(exprs) {
  const program = {
    type: 'Program',
    body: exprs.map(e => toStatement(compileExpr(e))),
    sourceType: 'module',
  };
  return generate(program, { indent: '  ' });
}
