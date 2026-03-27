/**
 * @module
 * lykn expansion pass.
 * Transforms reader AST into compiler-ready AST by resolving quasiquote,
 * quote, sugar forms (cons/list/car/cdr), and as patterns.
 */

// --- AST Node API ---

/** @type {number} */
let gensymCounter = 0;

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
    if (!arr || arr.type !== 'list') {
      throw new Error(`append: expected list node, got ${arr?.type ?? 'null'}`);
    }
    values.push(...arr.values);
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

// --- Quasiquote Expansion (Bawden's Algorithm) ---

function expandQuasiquote(form, depth) {
  // Self-evaluating
  if (form.type === 'number' || form.type === 'string') {
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
  "macroexpand":   { walk: "debug-expand" },
  "macroexpand-1": { walk: "debug-expand" },
};

// --- Expansion Walk ---

/**
 * Expand a single AST form, resolving sugar, quasiquote, and quote.
 * @param {*} form - A reader AST node
 * @returns {* | *[]} Expanded form(s)
 */
export function expandExpr(form) {
  if (form === null || form === undefined) return form;
  if (form.type === 'atom' || form.type === 'number' || form.type === 'string') {
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

  // Dispatch table
  if (head.type === 'atom') {
    const entry = dispatchTable[head.value];
    if (entry) {
      switch (entry.walk) {
        case 'none':
          return form;

        case 'register-macro':
          throw new Error('unexpected macro definition in expansion pass (macro processing not yet implemented)');

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

        case 'debug-expand':
          throw new Error(`${head.value} not yet implemented (requires macro system)`);

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
 * Expand all top-level forms.
 * @param {*[]} forms - Array of reader AST nodes
 * @returns {*[]} Expanded forms ready for the compiler
 */
export function expand(forms) {
  const result = [];
  for (const form of forms) {
    const expanded = expandExpr(form);
    if (Array.isArray(expanded)) {
      result.push(...expanded);
    } else {
      result.push(expanded);
    }
  }
  return result;
}
