/**
 * @module
 * lykn expansion pass.
 * Transforms reader AST into compiler-ready AST by resolving quasiquote,
 * quote, sugar forms (cons/list/car/cdr), macros, and as patterns.
 */

import { compile } from './compiler.js';
import { read } from './reader.js';
import { registerSurfaceMacros, resetTypeRegistry } from './surface.js';

// node:path is needed for import-macros file resolution.
// In browser builds, esbuild replaces this with stubs via the define/inject mechanism.
import { resolve as _resolve, dirname as _dirname } from "node:path";

// --- AST Node API ---

/** @type {number} */
let gensymCounter = 0;

/** @type {Map<string, Function>} */
const macroEnv = new Map();

/** @type {Map<string, { mtime: number, exports: Map<string, Function> }>} */
const moduleCache = new Map();

const MAX_EXPAND_ITERATIONS = 1000;

/**
 * Create a symbol (atom) AST node.
 * @param {string} name
 * @returns {{ type: 'atom', value: string }}
 */
export function sym(name) {
  return { type: 'atom', value: name };
}

/**
 * Create a unique symbol for hygienic macro expansion.
 * @param {string} [prefix='g']
 * @returns {{ type: 'atom', value: string }}
 */
export function gensym(prefix = 'g') {
  return { type: 'atom', value: `${prefix}__gensym${gensymCounter++}` };
}

/**
 * Reset gensym counter (for testing determinism).
 */
export function resetGensym() {
  gensymCounter = 0;
}

/**
 * Create a list AST node from items.
 * @param {...*} items - AST nodes
 * @returns {{ type: 'list', values: *[] }}
 */
export function array(...items) {
  return { type: 'list', values: items };
}

/**
 * Concatenate list node values into a single list node.
 * @param {...*} arrays - List nodes
 * @returns {{ type: 'list', values: *[] }}
 * @throws {Error} If any argument is not a list node
 */
export function append(...arrays) {
  const values = [];
  for (const arr of arrays) {
    if (Array.isArray(arr)) {
      // Raw JS array (e.g., from rest params in macro body)
      values.push(...arr);
    } else if (arr && arr.type === 'list') {
      values.push(...arr.values);
    } else {
      throw new Error(`append: expected list node or array, got ${arr?.type ?? 'null'}`);
    }
  }
  return { type: 'list', values };
}

/** @param {*} x @returns {boolean} */
export function isArray(x) {
  return x !== null && x !== undefined && x.type === 'list';
}

/** @param {*} x @returns {boolean} */
export function isSymbol(x) {
  return x !== null && x !== undefined && x.type === 'atom';
}

/** @param {*} x @returns {boolean} */
export function isNumber(x) {
  return x !== null && x !== undefined && x.type === 'number';
}

/** @param {*} x @returns {boolean} */
export function isString(x) {
  return x !== null && x !== undefined && x.type === 'string';
}

/** @param {*} x @returns {boolean} */
export function isKeyword(x) {
  return x !== null && x !== undefined && x.type === 'keyword';
}

/**
 * Get first element of a list node.
 * @param {{ type: 'list', values: *[] }} arr
 * @returns {* | undefined}
 */
export function first(arr) {
  return arr.values[0];
}

/**
 * Get all but first element as a new list node.
 * @param {{ type: 'list', values: *[] }} arr
 * @returns {{ type: 'list', values: *[] }}
 */
export function rest(arr) {
  return { type: 'list', values: arr.values.slice(1) };
}

/** Alias for append. */
export const concat = append;

/** @param {{ type: 'list', values: *[] }} arr @returns {number} */
export function length(arr) {
  return arr.values.length;
}

/** @param {{ type: 'list', values: *[] }} arr @param {number} n @returns {* | undefined} */
export function nth(arr, n) {
  return arr.values[n];
}

/**
 * Format an AST node as an s-expression string (for debug output).
 * @param {*} node
 * @returns {string}
 */
function formatSExpr(node) {
  if (node === null || node === undefined) return 'null';
  if (node.type === 'atom') return node.value;
  if (node.type === 'keyword') return `:${node.value}`;
  if (node.type === 'string') return `"${node.value}"`;
  if (node.type === 'number') return String(node.value);
  if (node.type === 'cons') return `(${formatSExpr(node.car)} . ${formatSExpr(node.cdr)})`;
  if (node.type === 'list') {
    return `(${node.values.map(formatSExpr).join(' ')})`;
  }
  return String(node);
}

// --- Sugar Transforms ---

function desugarCons(args) {
  if (args.length !== 2) {
    throw new Error('cons requires exactly 2 arguments');
  }
  return array(sym('array'), args[0], args[1]);
}

function desugarList(args) {
  if (args.length === 0) {
    return sym('null');
  }
  let result = sym('null');
  for (let i = args.length - 1; i >= 0; i--) {
    result = array(sym('array'), args[i], result);
  }
  return result;
}

function desugarCar(args) {
  if (args.length !== 1) throw new Error('car requires exactly 1 argument');
  return array(sym('get'), args[0], { type: 'number', value: 0 });
}

function desugarCdr(args) {
  if (args.length !== 1) throw new Error('cdr requires exactly 1 argument');
  return array(sym('get'), args[0], { type: 'number', value: 1 });
}

function desugarCadr(args) {
  if (args.length !== 1) throw new Error('cadr requires exactly 1 argument');
  return array(sym('get'),
    array(sym('get'), args[0], { type: 'number', value: 1 }),
    { type: 'number', value: 0 });
}

function desugarCddr(args) {
  if (args.length !== 1) throw new Error('cddr requires exactly 1 argument');
  return array(sym('get'),
    array(sym('get'), args[0], { type: 'number', value: 1 }),
    { type: 'number', value: 1 });
}

function desugarAs(args) {
  if (args.length !== 2) throw new Error('as requires exactly 2 arguments');
  if (args[0].type === 'atom') {
    return array(sym('alias'), args[0], args[1]);
  }
  throw new Error('as with pattern first argument must appear in binding position (const/let/var)');
}

// --- Macro Compilation ---

/** Macro environment API parameter names for new Function(). */
const MACRO_API_PARAMS = [
  '$array', '$sym', '$gensym',
  '$isArray', '$isSymbol', '$isNumber', '$isString', '$isKeyword',
  '$first', '$rest', '$concat', '$nth', '$length',
  '$append',
];

/** Macro environment API values, matching MACRO_API_PARAMS order. */
const MACRO_API_VALUES = [
  array, sym, gensym,
  isArray, isSymbol, isNumber, isString, isKeyword,
  first, rest, concat, nth, length,
  append,
];

/**
 * Compile a quasiquote template into s-expression AST that, when compiled
 * to JS and executed, constructs the template with unquoted values filled in.
 * This is different from expandQuasiquote which RESOLVES templates.
 * @param {*} form - The quasiquote body
 * @param {number} depth - Nesting depth
 * @returns {*} S-expression AST representing API calls
 */
function compileQuasiquote(form, depth) {
  if (form.type === 'number') {
    return form;
  }
  if (form.type === 'string') {
    return form;
  }
  if (form.type === 'keyword') {
    return form;
  }
  if (form.type === 'atom') {
    return array(sym('$sym'), { type: 'string', value: form.value });
  }

  if (form.type === 'cons') {
    const car = compileQuasiquote(form.car, depth);
    const cdr = compileQuasiquote(form.cdr, depth);
    return array(sym('$array'), car, cdr);
  }

  if (form.type !== 'list') {
    throw new Error(`compileQuasiquote: unexpected node type '${form.type}'`);
  }

  const values = form.values;
  if (values.length === 0) {
    return array(sym('$array'));
  }

  const head = values[0];

  if (head.type === 'atom' && head.value === 'quasiquote') {
    if (values.length !== 2) throw new Error('quasiquote requires exactly one argument');
    const inner = compileQuasiquote(values[1], depth + 1);
    return array(sym('$array'), array(sym('$sym'), { type: 'string', value: 'quasiquote' }), inner);
  }

  if (head.type === 'atom' && head.value === 'unquote') {
    if (values.length !== 2) throw new Error('unquote requires exactly one argument');
    if (depth === 0) {
      return values[1];
    }
    const inner = compileQuasiquote(values[1], depth - 1);
    return array(sym('$array'), array(sym('$sym'), { type: 'string', value: 'unquote' }), inner);
  }

  if (head.type === 'atom' && head.value === 'unquote-splicing') {
    if (depth === 0) {
      throw new Error('unquote-splicing not inside a list');
    }
    if (values.length !== 2) throw new Error('unquote-splicing requires exactly one argument');
    const inner = compileQuasiquote(values[1], depth - 1);
    return array(sym('$array'), array(sym('$sym'), { type: 'string', value: 'unquote-splicing' }), inner);
  }

  const parts = values.map((el) => compileQQElementForMacro(el, depth));
  const hasSplice = parts.some((p) => p.isSplice);

  if (!hasSplice) {
    return array(sym('$array'), ...parts.map((p) => p.node));
  }

  const concatArgs = parts.map((p) => {
    if (p.isSplice) return p.node;
    return array(sym('$array'), p.node);
  });
  return array(sym('$concat'), ...concatArgs);
}

function compileQQElementForMacro(element, depth) {
  if (element.type === 'list' && element.values.length === 2 &&
      element.values[0].type === 'atom' && element.values[0].value === 'unquote') {
    if (depth === 0) {
      return { node: element.values[1], isSplice: false };
    }
    return { node: compileQuasiquote(element, depth), isSplice: false };
  }

  if (element.type === 'list' && element.values.length === 2 &&
      element.values[0].type === 'atom' && element.values[0].value === 'unquote-splicing') {
    if (depth === 0) {
      return { node: element.values[1], isSplice: true };
    }
    return { node: compileQuasiquote(element, depth), isSplice: false };
  }

  if (element.type === 'list') {
    return { node: compileQuasiquote(element, depth), isSplice: false };
  }

  if (element.type === 'number' || element.type === 'string') {
    return { node: element, isSplice: false };
  }

  if (element.type === 'atom') {
    return { node: array(sym('$sym'), { type: 'string', value: element.value }), isSplice: false };
  }

  if (element.type === 'cons') {
    return { node: compileQuasiquote(element, depth), isSplice: false };
  }

  return { node: element, isSplice: false };
}

/**
 * Resolve #gen auto-gensym suffixes in a quasiquote template.
 * All occurrences of the same prefix#gen within one template → same gensym.
 * @param {*} form - AST form (quasiquote body)
 * @returns {*} Form with #gen atoms replaced by gensym atoms
 */
function resolveAutoGensym(form) {
  const genMap = new Map();

  function resolve(node) {
    if (node === null || node === undefined) return node;

    if (node.type === 'atom' && node.value.endsWith('#gen')) {
      const prefix = node.value.slice(0, -4);
      if (!genMap.has(prefix)) {
        genMap.set(prefix, gensym(prefix));
      }
      return genMap.get(prefix);
    }

    if (node.type === 'list') {
      return { type: 'list', values: node.values.map(resolve) };
    }

    if (node.type === 'cons') {
      return { type: 'cons', car: resolve(node.car), cdr: resolve(node.cdr) };
    }

    return node;
  }

  return resolve(form);
}

/**
 * Compile a macro body into a JS function string.
 * The body may contain quasiquote templates which are compiled to API calls.
 * @param {*[]} paramNames - Extracted parameter name atoms
 * @param {*} paramPattern - The raw parameter pattern for destructuring
 * @param {*[]} bodyForms - The macro body forms
 * @returns {string} JS code string for new Function()
 */
function compileMacroBody(_paramNames, paramPattern, bodyForms) {
  // Process the body: compile quasiquote templates to API calls
  const processedBody = bodyForms.map((form) => processBodyForm(form));

  // Build a function that takes the call-site args and returns an s-expression
  // For simple (test (rest body)): function(test, ...body) { return <compiled-body>; }
  const jsParams = compileParamList(paramPattern);

  // The last expression is the return value
  const bodyStatements = processedBody.map((form, i) => {
    if (i === processedBody.length - 1) {
      return array(sym('return'), form);
    }
    return form;
  });

  // Use lambda (FunctionExpression) for proper block body with return
  const fnForm = array(sym('lambda'), array(...jsParams), ...bodyStatements);

  // Compile to JS using the compiler
  const jsCode = compile([fnForm]);
  return `return ${jsCode.trim()};`;
}

/**
 * Process a macro body form, converting quasiquote templates to API calls.
 */
function processBodyForm(form) {
  if (form === null || form === undefined) return form;

  if (form.type !== 'list' || form.values.length === 0) return form;

  const head = form.values[0];

  // Quasiquote in macro body → compile to API calls (not resolve)
  if (head.type === 'atom' && head.value === 'quasiquote') {
    if (form.values.length !== 2) throw new Error('quasiquote requires exactly one argument');
    const body = resolveAutoGensym(form.values[1]);
    return compileQuasiquote(body, 0);
  }

  // Recursively process sub-forms
  return { type: 'list', values: form.values.map(processBodyForm) };
}

/**
 * Convert a macro param pattern to JS parameter atoms.
 * (test (rest body)) → [sym('test'), sym('...body')] for arrow fn compilation
 */
function compileParamList(pattern) {
  if (pattern.type !== 'list') return [];

  const params = [];
  for (const p of pattern.values) {
    if (p.type === 'atom') {
      if (p.value === '_') {
        params.push(sym('_'));
      } else {
        params.push(p);
      }
    } else if (p.type === 'list' && p.values.length >= 1) {
      const head = p.values[0];
      if (head.type === 'atom' && head.value === 'rest') {
        // (rest body) → ...body in JS (rest parameter)
        params.push(p); // pass through — compilePattern handles (rest x)
      } else if (head.type === 'atom' && head.value === 'default') {
        params.push(p); // pass through — compilePattern handles (default x val)
      } else {
        params.push(p); // nested pattern — compilePattern handles it
      }
    }
  }
  return params;
}

/**
 * Register a macro from a (macro name params body...) form.
 * @param {*} nameNode - The macro name atom
 * @param {*} paramsNode - The parameter pattern list
 * @param {*[]} bodyForms - The body forms
 * @throws {Error} If macro compilation fails
 */
function registerMacroForm(nameNode, paramsNode, bodyForms) {
  const name = nameNode.value;

  if (macroEnv.has(name)) {
    throw new Error(`duplicate macro definition: '${name}'`);
  }

  const paramNames = extractParamNames(paramsNode);
  const jsBody = compileMacroBody(paramNames, paramsNode, bodyForms);

  try {
    const factory = new Function(...MACRO_API_PARAMS, jsBody);
    const macroFn = factory(...MACRO_API_VALUES);
    macroEnv.set(name, macroFn);
  } catch (err) {
    throw new Error(`failed to compile macro '${name}': ${err.message}`, { cause: err });
  }
}

/**
 * Extract parameter names from a macro param pattern (for gensym checking).
 */
function extractParamNames(pattern) {
  const names = new Set();

  function walk(node) {
    if (node === null || node === undefined) return;
    if (node.type === 'atom' && node.value !== '_') {
      names.add(node.value);
    } else if (node.type === 'list') {
      const head = node.values[0];
      if (head?.type === 'atom' && head.value === 'rest' && node.values[1]) {
        walk(node.values[1]);
      } else if (head?.type === 'atom' && head.value === 'default' && node.values[1]) {
        walk(node.values[1]);
      } else {
        for (const child of node.values) walk(child);
      }
    }
  }

  if (pattern.type === 'list') {
    for (const p of pattern.values) walk(p);
  }
  return names;
}

/**
 * Reset macro environment (for testing).
 */
export function resetMacros() {
  macroEnv.clear();
  resetTypeRegistry();
  registerSurfaceMacros(macroEnv);
}

export function resetModuleCache() {
  moduleCache.clear();
}

export { formatSExpr, extractParamNames, compileMacroBody };

// --- Quasiquote Expansion (Bawden's Algorithm) ---

function expandQuasiquote(form, depth) {
  // Self-evaluating
  if (form.type === 'number' || form.type === 'string' || form.type === 'keyword') {
    return form;
  }

  // Atoms: quote them
  if (form.type === 'atom') {
    return array(sym('quote'), form);
  }

  // Cons node: expand car and cdr, wrap as array
  if (form.type === 'cons') {
    const expandedCar = expandQuasiquote(form.car, depth);
    const expandedCdr = expandQuasiquote(form.cdr, depth);
    return array(sym('array'), expandedCar, expandedCdr);
  }

  if (form.type !== 'list') {
    throw new Error(`expandQuasiquote: unexpected node type '${form.type}'`);
  }

  const values = form.values;

  // Empty list
  if (values.length === 0) {
    return form;
  }

  const head = values[0];

  // Nested quasiquote: increment depth
  if (head.type === 'atom' && head.value === 'quasiquote') {
    if (values.length !== 2) throw new Error('quasiquote requires exactly one argument');
    const expanded = expandQuasiquote(values[1], depth + 1);
    return array(sym('array'), array(sym('quote'), sym('quasiquote')), expanded);
  }

  // Unquote
  if (head.type === 'atom' && head.value === 'unquote') {
    if (values.length !== 2) throw new Error('unquote requires exactly one argument');
    if (depth === 0) {
      return values[1];
    }
    const expanded = expandQuasiquote(values[1], depth - 1);
    return array(sym('array'), array(sym('quote'), sym('unquote')), expanded);
  }

  // Unquote-splicing as direct child of quasiquote (not in list)
  if (head.type === 'atom' && head.value === 'unquote-splicing') {
    if (depth === 0) {
      throw new Error('unquote-splicing not inside a list');
    }
    if (values.length !== 2) throw new Error('unquote-splicing requires exactly one argument');
    const expanded = expandQuasiquote(values[1], depth - 1);
    return array(sym('array'), array(sym('quote'), sym('unquote-splicing')), expanded);
  }

  // General list: expand each element
  const parts = values.map((el) => expandQQElement(el, depth));

  // Optimization: all literal → return form as direct structure
  if (parts.every((p) => p.isLiteral)) {
    return quoteLiteral(form);
  }

  // Optimization: no splices → use array directly
  if (!parts.some((p) => p.isSplice)) {
    return array(sym('array'), ...parts.map((p) => p.node));
  }

  // General case with splices: use append
  const appendArgs = parts.map((p) => {
    if (p.isSplice) return p.node;
    return array(sym('array'), p.node);
  });
  return array(sym('append'), ...appendArgs);
}

function expandQQElement(element, depth) {
  // Unquote
  if (element.type === 'list' && element.values.length === 2 &&
      element.values[0].type === 'atom' && element.values[0].value === 'unquote') {
    if (depth === 0) {
      return { node: element.values[1], isSplice: false, isLiteral: false };
    }
    return { node: expandQuasiquote(element, depth), isSplice: false, isLiteral: false };
  }

  // Unquote-splicing
  if (element.type === 'list' && element.values.length === 2 &&
      element.values[0].type === 'atom' && element.values[0].value === 'unquote-splicing') {
    if (depth === 0) {
      return { node: element.values[1], isSplice: true, isLiteral: false };
    }
    return { node: expandQuasiquote(element, depth), isSplice: false, isLiteral: false };
  }

  // Nested list: recurse
  if (element.type === 'list') {
    return { node: expandQuasiquote(element, depth), isSplice: false, isLiteral: false };
  }

  // Self-evaluating literals
  if (element.type === 'number' || element.type === 'string') {
    return { node: element, isSplice: false, isLiteral: true };
  }

  // Atom: quote it
  if (element.type === 'atom') {
    return { node: array(sym('quote'), element), isSplice: false, isLiteral: true };
  }

  // Cons node
  if (element.type === 'cons') {
    return { node: expandQuasiquote(element, depth), isSplice: false, isLiteral: false };
  }

  return { node: element, isSplice: false, isLiteral: true };
}

function quoteLiteral(form) {
  if (form.type === 'number' || form.type === 'string') return form;
  if (form.type === 'atom') return array(sym('quote'), form);
  if (form.type === 'list') {
    if (form.values.length === 0) return form;
    return array(...form.values.map(quoteLiteral));
  }
  if (form.type === 'cons') {
    return array(quoteLiteral(form.car), quoteLiteral(form.cdr));
  }
  return form;
}

// --- Dispatch Table ---

const dispatchTable = {
  "quote":       { walk: "none" },
  "macro":       { walk: "register-macro" },
  "cons":        { walk: "desugar", transform: desugarCons },
  "list":        { walk: "desugar", transform: desugarList },
  "car":         { walk: "desugar", transform: desugarCar },
  "cdr":         { walk: "desugar", transform: desugarCdr },
  "cadr":        { walk: "desugar", transform: desugarCadr },
  "cddr":        { walk: "desugar", transform: desugarCddr },
  "as":          { walk: "desugar", transform: desugarAs },
  "const":       { walk: "expand-binding", keyword: "const" },
  "let":         { walk: "expand-binding", keyword: "let" },
  "var":         { walk: "expand-binding", keyword: "var" },
  "import-macros": { walk: "import-macros" },
  "macroexpand":   { walk: "debug-expand", mode: "full" },
  "macroexpand-1": { walk: "debug-expand", mode: "once" },
};

// --- Expansion Walk ---

/**
 * Expand a single AST form, resolving sugar, quasiquote, and quote.
 * @param {*} form - A reader AST node
 * @returns {* | *[]} Expanded form(s)
 */
export function expandExpr(form) {
  if (form === null || form === undefined) return form;
  if (form.type === 'atom' || form.type === 'number' || form.type === 'string' || form.type === 'keyword') {
    return form;
  }

  if (form.type === 'cons') {
    return { type: 'cons', car: expandExpr(form.car), cdr: expandExpr(form.cdr) };
  }

  if (form.type !== 'list') {
    throw new Error(`expandExpr: unexpected node type '${form.type}'`);
  }

  if (form.values.length === 0) return form;

  const head = form.values[0];

  // Quasiquote
  if (head.type === 'atom' && head.value === 'quasiquote') {
    if (form.values.length !== 2) throw new Error('quasiquote requires exactly one argument');
    const expanded = expandQuasiquote(form.values[1], 0);
    return expandExpr(expanded);
  }

  // Unquote/splice outside quasiquote
  if (head.type === 'atom' && head.value === 'unquote') {
    throw new Error('unquote outside of quasiquote');
  }
  if (head.type === 'atom' && head.value === 'unquote-splicing') {
    throw new Error('unquote-splicing outside of quasiquote');
  }

  // Fixed-point macro expansion
  // Skip re-expansion of forms marked as kernel output by surface macros
  if (head.type === 'atom' && macroEnv.has(head.value) && !form._kernel) {
    let current = form;
    let count = 0;
    while (current.type === 'list' && current.values.length > 0 &&
           current.values[0].type === 'atom' && macroEnv.has(current.values[0].value) &&
           !current._kernel) {
      const macroName = current.values[0].value;
      const macroArgs = current.values.slice(1);
      try {
        current = macroEnv.get(macroName)(...macroArgs);
      } catch (err) {
        throw new Error(`error expanding macro '${macroName}': ${err.message}`, { cause: err });
      }
      if (++count > MAX_EXPAND_ITERATIONS) {
        throw new Error(`expansion limit (${MAX_EXPAND_ITERATIONS}) exceeded expanding '${form.values[0].value}'`);
      }
    }
    return expandExpr(current);
  }

  // Dispatch table
  if (head.type === 'atom') {
    const entry = dispatchTable[head.value];
    if (entry) {
      switch (entry.walk) {
        case 'none':
          return form;

        case 'register-macro':
          throw new Error('unexpected macro definition in expansion pass (macros should be processed in Pass 1)');

        case 'desugar': {
          const args = form.values.slice(1);
          const result = entry.transform(args);
          if (Array.isArray(result)) {
            return result.map((r) => expandExpr(r));
          }
          return expandExpr(result);
        }

        case 'expand-binding': {
          const args = form.values.slice(1);
          // Check for (as pattern whole) in binding position
          if (args[0]?.type === 'list' && args[0].values.length >= 1 &&
              args[0].values[0]?.type === 'atom' && args[0].values[0]?.value === 'as') {
            const asArgs = args[0].values.slice(1);
            if (asArgs.length === 2 && asArgs[0].type === 'list') {
              const pattern = asArgs[0];
              const whole = asArgs[1];
              const initExpr = args[1];
              return [
                expandExpr(array(sym(entry.keyword), whole, initExpr)),
                expandExpr(array(sym(entry.keyword), pattern, whole)),
              ];
            }
          }
          // No as pattern — default recursive expansion
          return { type: 'list', values: form.values.map((sub) => expandExpr(sub)) };
        }

        case 'import-macros':
          throw new Error('import-macros not yet implemented (Phase 5)');

        case 'debug-expand': {
          const debugArgs = form.values.slice(1);
          if (debugArgs.length !== 1) {
            throw new Error(`${head.value} requires exactly one argument (quoted form)`);
          }
          let targetForm = debugArgs[0];
          // Strip (quote ...) wrapper if present
          if (targetForm.type === 'list' && targetForm.values.length === 2 &&
              targetForm.values[0].type === 'atom' && targetForm.values[0].value === 'quote') {
            targetForm = targetForm.values[1];
          }

          let result;
          if (entry.mode === 'once') {
            // macroexpand-1: one expansion step only
            if (targetForm.type === 'list' && targetForm.values.length > 0 &&
                targetForm.values[0].type === 'atom' && macroEnv.has(targetForm.values[0].value)) {
              const macroName = targetForm.values[0].value;
              const macroArgs = targetForm.values.slice(1);
              result = macroEnv.get(macroName)(...macroArgs);
            } else {
              result = targetForm;
            }
          } else {
            // macroexpand: full expansion
            result = expandExpr(targetForm);
          }

          // Print to stderr
          console.error(formatSExpr(result));
          // Erase from output
          return null;
        }

        default:
          throw new Error(`unknown dispatch walk strategy: '${entry.walk}'`);
      }
    }
  }

  // Default: expand all sub-forms
  const expandedValues = [];
  for (const sub of form.values) {
    const result = expandExpr(sub);
    if (Array.isArray(result)) {
      expandedValues.push(...result);
    } else {
      expandedValues.push(result);
    }
  }
  return { type: 'list', values: expandedValues };
}

/**
 * Pass 1: Scan for macro definitions, compile and register them.
 * Uses iterative fixed-point for order-independent macro compilation.
 * @param {*[]} forms - All top-level forms
 * @returns {*[]} Forms with macro definitions removed
 */
function pass1RegisterMacros(forms) {
  const macroForms = [];
  const otherForms = [];

  for (const form of forms) {
    if (form.type === 'list' && form.values.length >= 3 &&
        form.values[0].type === 'atom' && form.values[0].value === 'macro') {
      macroForms.push(form);
    } else {
      otherForms.push(form);
    }
  }

  if (macroForms.length === 0) return otherForms;

  // Iterative fixed-point: compile macros in dependency order
  let pending = [...macroForms];
  const maxPasses = pending.length;
  let passCount = 0;

  while (pending.length > 0) {
    passCount++;
    if (passCount > maxPasses) {
      const names = pending.map((f) => f.values[1].value).join(', ');
      throw new Error(`circular macro dependency among: ${names}`);
    }

    let progress = false;
    const stillPending = [];

    for (const form of pending) {
      const name = form.values[1];
      const params = form.values[2];
      const body = form.values.slice(3);

      // Check if body references any still-pending macro names
      const pendingNames = new Set(pending.map((f) => f.values[1].value));
      pendingNames.delete(name.value); // Don't count self-reference
      const deps = findSymbolRefs(body, pendingNames);

      if (deps.size === 0) {
        registerMacroForm(name, params, body);
        progress = true;
      } else {
        stillPending.push(form);
      }
    }

    pending = stillPending;
    if (!progress && pending.length > 0) {
      const names = pending.map((f) => f.values[1].value).join(', ');
      throw new Error(`circular macro dependency among: ${names}`);
    }
  }

  return otherForms;
}

/**
 * Find references to a set of symbol names within forms.
 * @param {*[]} forms - Forms to search
 * @param {Set<string>} names - Symbol names to look for
 * @returns {Set<string>} Found references
 */
function findSymbolRefs(forms, names) {
  const found = new Set();

  function walk(node) {
    if (node === null || node === undefined) return;
    if (node.type === 'atom' && names.has(node.value)) {
      found.add(node.value);
    } else if (node.type === 'list') {
      for (const child of node.values) walk(child);
    } else if (node.type === 'cons') {
      walk(node.car);
      walk(node.cdr);
    }
  }

  for (const form of forms) walk(form);
  return found;
}

/**
 * Pass 2: Expand all forms (sugar, quasiquote, macros).
 * @param {*[]} forms - Forms after Pass 1 (macros removed)
 * @returns {*[]} Fully expanded forms
 */
function pass2ExpandAll(forms) {
  const result = [];
  for (const form of forms) {
    const expanded = expandExpr(form);
    if (expanded === null || expanded === undefined) continue;
    if (Array.isArray(expanded)) {
      result.push(...expanded.filter((e) => e !== null && e !== undefined));
    } else {
      result.push(expanded);
    }
  }
  return result;
}

/**
 * Find the project.json import map by walking up from a starting directory.
 * @param {string | null} filePath - Path of the importing file
 * @returns {{ projectRoot: string, imports: Object } | null}
 */
function findProjectImports(filePath) {
  const startDir = filePath ? _dirname(filePath) : (typeof Deno !== 'undefined' ? Deno.cwd() : '.');
  let dir = startDir;
  const root = _resolve('/');

  while (dir !== root) {
    const projectJson = _resolve(dir, 'project.json');
    try {
      const content = Deno.readTextFileSync(projectJson);
      const config = JSON.parse(content);
      return { projectRoot: dir, imports: config.imports || {} };
    } catch {
      dir = _dirname(dir);
    }
  }
  return null;
}

/**
 * Find the macro entry file in a package directory.
 * Checks: lykn.macroEntry field, then mod.lykn, macros.lykn, index.lykn,
 * then exports if it points to .lykn.
 * @param {string} pkgDir - Absolute path to the package directory
 * @returns {string} Absolute path to the macro entry file
 */
function findMacroEntry(pkgDir) {
  const denoJsonPath = _resolve(pkgDir, 'deno.json');
  try {
    const content = Deno.readTextFileSync(denoJsonPath);
    const config = JSON.parse(content);
    if (config.lykn?.macroEntry) {
      const entryPath = _resolve(pkgDir, config.lykn.macroEntry);
      try { Deno.statSync(entryPath); return entryPath; } catch { /* file not found */ }
    }
    // Fallback chain
    for (const candidate of ['mod.lykn', 'macros.lykn', 'index.lykn']) {
      const p = _resolve(pkgDir, candidate);
      try { Deno.statSync(p); return p; } catch { /* file not found */ }
    }
    // Check exports
    if (typeof config.exports === 'string' && config.exports.endsWith('.lykn')) {
      const p = _resolve(pkgDir, config.exports);
      try { Deno.statSync(p); return p; } catch { /* file not found */ }
    }
  } catch { /* no deno.json */ }

  throw new Error(
    `import-macros: no macro entry found in ${pkgDir}\n` +
    `  checked: lykn.macroEntry, mod.lykn, macros.lykn, index.lykn\n` +
    `  hint: add lykn.macroEntry to the package's deno.json`
  );
}

/**
 * Three-tier specifier resolution for import-macros.
 * Tier 1: scheme-prefixed (jsr:, npm:, https:, file:) → Deno's resolver
 * Tier 2: bare name → import-map lookup from project.json
 * Tier 3: filesystem path (relative/absolute)
 * @param {string} specifier
 * @param {string | null} filePath
 * @returns {string} Resolved absolute path
 */
function resolveImportMacrosSpecifier(specifier, filePath) {
  // Tier 1: Scheme-prefixed — use Deno's resolver
  if (/^(jsr|npm|https?):/.test(specifier)) {
    try {
      const resolved = import.meta.resolve(specifier);
      if (resolved.startsWith('file://')) {
        let fsPath = new URL(resolved).pathname;
        // If it points to a file, get the directory
        if (/\.[a-z]+$/i.test(fsPath)) {
          fsPath = _dirname(fsPath);
        }
        return findMacroEntry(fsPath);
      }
      throw new Error(`import-macros: resolved ${specifier} to non-file URL: ${resolved}`);
    } catch (e) {
      if (e.message?.startsWith('import-macros:')) throw e;
      throw new Error(`import-macros: could not resolve "${specifier}": ${e.message}`);
    }
  }

  // file: scheme
  if (specifier.startsWith('file://')) {
    const fsPath = new URL(specifier).pathname;
    return fsPath;
  }

  // Tier 2: Import-map lookup (bare name or prefix match)
  if (!specifier.startsWith('./') && !specifier.startsWith('../') && !specifier.startsWith('/')) {
    const project = findProjectImports(filePath);
    if (project) {
      const { projectRoot, imports } = project;

      // Exact match
      if (imports[specifier]) {
        const target = imports[specifier];
        if (/^(jsr|npm|https?):/.test(target)) {
          return resolveImportMacrosSpecifier(target, filePath);
        }
        if (target.startsWith('./') || target.startsWith('../')) {
          const resolved = _resolve(projectRoot, target);
          try {
            const stat = Deno.statSync(resolved);
            if (stat.isDirectory) return findMacroEntry(resolved);
            return resolved;
          } catch {
            throw new Error(`import-macros: import map target "${target}" for "${specifier}" not found`);
          }
        }
      }

      // Prefix match: find longest key ending in '/' that matches
      let bestKey = null;
      for (const key of Object.keys(imports)) {
        if (key.endsWith('/') && specifier.startsWith(key)) {
          if (!bestKey || key.length > bestKey.length) bestKey = key;
        }
      }
      if (bestKey) {
        const suffix = specifier.slice(bestKey.length);
        const target = imports[bestKey];
        if (target.startsWith('./') || target.startsWith('../')) {
          return _resolve(projectRoot, target, suffix);
        }
        return resolveImportMacrosSpecifier(`${target}${suffix}`, filePath);
      }

      // Workspace package fallback: try packages/<name>/mod.lykn
      const modPath = _resolve(projectRoot, 'packages', specifier, 'mod.lykn');
      try { Deno.statSync(modPath); return modPath; } catch { /* not found */ }
    }
  }

  // Tier 3: Filesystem path (current behavior)
  if (specifier.startsWith('./') || specifier.startsWith('../')) {
    if (!specifier.endsWith('.lykn')) {
      throw new Error(`import-macros path must end with .lykn: "${specifier}"`);
    }
    const baseDir = filePath ? _dirname(filePath) : (typeof Deno !== 'undefined' ? Deno.cwd() : '.');
    return _resolve(baseDir, specifier);
  }

  throw new Error(
    `import-macros: could not resolve "${specifier}"\n` +
    `  tier 2 (project.json imports): no matching key\n` +
    `  tier 3 (filesystem): not a relative path\n` +
    `  hint: add an entry to project.json "imports" or use a scheme prefix (jsr:, npm:)`
  );
}

/**
 * Pass 0: Process import-macros forms. Load, compile, and register
 * macros from external .lykn modules.
 * @param {*[]} forms - All top-level forms
 * @param {string | null} filePath - Path of the importing file (for resolution)
 * @param {string[]} compilationStack - Stack for circular dep detection
 * @returns {*[]} Forms with import-macros removed
 */
function pass0ImportMacros(forms, filePath, compilationStack) {
  const remaining = [];
  const importedPaths = new Set();

  for (const form of forms) {
    if (form.type !== 'list' || form.values.length < 1 ||
        form.values[0].type !== 'atom' || form.values[0].value !== 'import-macros') {
      remaining.push(form);
      continue;
    }

    // (import-macros "path" (bindings...))
    const args = form.values.slice(1);
    if (args.length < 2) {
      throw new Error('import-macros requires a path and binding list');
    }
    if (args[0].type !== 'string') {
      throw new Error('import-macros: first argument must be a module path string');
    }
    if (args[1].type !== 'list') {
      throw new Error('import-macros requires a binding list');
    }

    const modulePath = args[0].value;

    // Resolve path (requires node:path — not available in browser)
    if (!_resolve || !_dirname) {
      throw new Error('import-macros requires Deno/Node file system access — not available in browser');
    }

    const resolvedPath = resolveImportMacrosSpecifier(modulePath, filePath);

    // Duplicate check
    if (importedPaths.has(resolvedPath)) {
      throw new Error(`duplicate import-macros for ${modulePath}`);
    }
    importedPaths.add(resolvedPath);

    // Load module
    const { macros: moduleMacros, runtimeImports } = loadMacroModule(resolvedPath, modulePath, compilationStack);

    // Register requested bindings
    const bindings = args[1].values;
    for (const binding of bindings) {
      let importedName;
      let localName;

      if (binding.type === 'atom') {
        importedName = binding.value;
        localName = binding.value;
      } else if (binding.type === 'list' && binding.values.length >= 2 &&
                 binding.values[0].type === 'atom' && binding.values[0].value === 'as') {
        importedName = binding.values[1].value;
        localName = binding.values[2].value;
      } else {
        throw new Error(`import-macros: invalid binding form`);
      }

      if (!moduleMacros.has(importedName)) {
        const available = [...moduleMacros.keys()].join(', ');
        throw new Error(
          `macro '${importedName}' not exported by ${modulePath}` +
          (available ? ` (available: ${available})` : '')
        );
      }

      if (macroEnv.has(localName)) {
        throw new Error(`macro '${localName}' already defined (imported from ${modulePath})`);
      }

      macroEnv.set(localName, moduleMacros.get(importedName));
    }

    // Emit runtime imports declared by the macro module
    remaining.push(...runtimeImports);
  }

  return remaining;
}

/**
 * Load and compile a macro module, returning its exported macro functions
 * and any runtime import declarations.
 * @param {string} resolvedPath - Absolute path to the .lykn file
 * @param {string} displayPath - Original relative path for error messages
 * @param {string[]} compilationStack - For circular dep detection
 * @returns {{ macros: Map<string, Function>, runtimeImports: *[] }}
 */
function loadMacroModule(resolvedPath, displayPath, compilationStack) {
  // Circular dependency check
  if (compilationStack.includes(resolvedPath)) {
    const cycle = [...compilationStack, resolvedPath].map((p) => p.split('/').pop()).join(' → ');
    throw new Error(`circular macro module dependency: ${cycle}`);
  }

  // Cache check
  let mtime;
  try {
    mtime = Deno.statSync(resolvedPath).mtime?.getTime() ?? 0;
  } catch {
    throw new Error(`macro module not found: ${displayPath}`);
  }

  const cached = moduleCache.get(resolvedPath);
  if (cached && cached.mtime === mtime) {
    return { macros: cached.macros, runtimeImports: cached.runtimeImports };
  }

  // Read and parse
  let source;
  try {
    source = Deno.readTextFileSync(resolvedPath);
  } catch {
    throw new Error(`macro module not found: ${displayPath}`);
  }

  const forms = read(source);

  // Save and clear current macro env (module gets its own scope)
  const savedMacroEnv = new Map(macroEnv);
  macroEnv.clear();

  const newStack = [...compilationStack, resolvedPath];

  try {
    // Run three-pass pipeline on module (recursive)
    const afterPass0 = pass0ImportMacros(forms, resolvedPath, newStack);

    // Pass 1: register macros, track which are exported
    const exportedMacroNames = new Set();
    const macroForms = [];
    const runtimeImports = [];
    const otherForms = [];

    for (const form of afterPass0) {
      // (export (macro name params body...))
      if (form.type === 'list' && form.values.length === 2 &&
          form.values[0].type === 'atom' && form.values[0].value === 'export' &&
          form.values[1].type === 'list' && form.values[1].values.length >= 3 &&
          form.values[1].values[0].type === 'atom' && form.values[1].values[0].value === 'macro') {
        const macroForm = form.values[1];
        const macroName = macroForm.values[1].value;
        exportedMacroNames.add(macroName);
        macroForms.push(macroForm);
      } else if (form.type === 'list' && form.values.length >= 3 &&
                 form.values[0].type === 'atom' && form.values[0].value === 'macro') {
        macroForms.push(form);
      } else if (form.type === 'list' && form.values.length >= 2 &&
                 form.values[0].type === 'atom' && form.values[0].value === 'runtime-import') {
        // (runtime-import "path" (bindings...)) → emitted as (import ...) in consuming file
        const riArgs = form.values.slice(1);
        if (riArgs[0]?.type !== 'string') {
          throw new Error('runtime-import: first argument must be a module path string');
        }
        runtimeImports.push(array(sym('import'), ...riArgs));
      } else if (form.type === 'list' && form.values.length === 2 &&
                 form.values[0].type === 'atom' && form.values[0].value === 'surface-macros' &&
                 form.values[1].type === 'string') {
        // (surface-macros "macros.js") → load JS companion that registers surface macros
        const jsFile = form.values[1].value;
        const jsPath = _resolve(_dirname(resolvedPath), jsFile);
        let jsSource;
        try { jsSource = Deno.readTextFileSync(jsPath); }
        catch { throw new Error(`surface-macros: file not found: ${jsFile}`); }
        const SURFACE_PARAMS = ['macroEnv', 'sym', 'array', 'gensym', 'isArray', 'isSymbol', 'isNumber', 'isString', 'isKeyword', 'first', 'rest', 'nth', 'length', 'append', 'formatSExpr'];
        const SURFACE_VALUES = [macroEnv, sym, array, gensym, isArray, isSymbol, isNumber, isString, isKeyword, first, rest, nth, length, append, formatSExpr];
        const beforeKeys = new Set(macroEnv.keys());
        try {
          const loader = new Function(...SURFACE_PARAMS, jsSource);
          loader(...SURFACE_VALUES);
        } catch (err) {
          throw new Error(`surface-macros: failed to load ${jsFile}: ${err.message}`, { cause: err });
        }
        for (const k of macroEnv.keys()) {
          if (!beforeKeys.has(k)) exportedMacroNames.add(k);
        }
      } else {
        otherForms.push(form);
      }
    }

    // Register all macros (exported and unexported)
    for (const form of macroForms) {
      const name = form.values[1];
      const params = form.values[2];
      const body = form.values.slice(3);
      registerMacroForm(name, params, body);
    }

    // Collect exported macro functions
    const macros = new Map();
    for (const name of exportedMacroNames) {
      if (macroEnv.has(name)) {
        macros.set(name, macroEnv.get(name));
      }
    }

    // Cache the result
    moduleCache.set(resolvedPath, { mtime, macros, runtimeImports });

    return { macros, runtimeImports };
  } finally {
    // Restore parent macro env
    macroEnv.clear();
    for (const [k, v] of savedMacroEnv) {
      macroEnv.set(k, v);
    }
  }
}

/**
 * Expand all top-level forms. Three-pass pipeline:
 * Pass 0: process import-macros (load external macro modules)
 * Pass 1: register macros (iterative fixed-point)
 * Pass 2: expand all remaining forms
 * @param {*[]} forms - Array of reader AST nodes
 * @param {{ filePath?: string, compilationStack?: string[] }} [context]
 * @returns {*[]} Expanded forms ready for the compiler
 */
export function expand(forms, context = {}) {
  // Ensure surface macros are registered (idempotent — skips if already present)
  if (!macroEnv.has('bind')) {
    registerSurfaceMacros(macroEnv);
  }
  const { filePath = null, compilationStack = [] } = context;
  const afterPass0 = pass0ImportMacros(forms, filePath, compilationStack);
  const afterPass1 = pass1RegisterMacros(afterPass0);
  return pass2ExpandAll(afterPass1);
}
