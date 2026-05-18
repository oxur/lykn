// DD-37 JS-side classifier (Phase 0 pilot).
// Pass-through wrapper around existing surface dispatch.
// Currently only handles `not`; other forms pass through to the
// existing surface macro path in expander.js/surface.js.

import { Not, Swap, Reset, SetProp, SetSymbol, Conj, Assoc, Dissoc, Thread, SomeThread, IfLet, WhenLet } from "./surface-ast.js";
import { compileLetPattern, wrapReturnLast, formatSExpr } from "./surface-helpers.js";

/**
 * Classify a surface form head atom. Returns a typed AST node if the
 * form is handled by the new architecture, or null to fall through to
 * the existing surface macro path.
 *
 * @param {string} head - the head atom of the form
 * @param {Array} args - the argument S-expressions
 * @returns {object|null} typed AST node, or null for fallthrough
 */
export function classifySurfaceForm(head, args) {
  switch (head) {
    case "not":
      if (args.length !== 1) {
        throw new Error("not requires exactly 1 argument: (not x)");
      }
      return Not(args[0]);
    case "swap!":
      if (args.length < 2) throw new Error("swap! requires at least 2 arguments: (swap! cell fn)");
      if (args[0].type !== "atom") throw new Error("swap!: first argument must be a symbol");
      return Swap(args[0], args[1], args.slice(2));
    case "reset!":
      if (args.length !== 2) throw new Error("reset! requires exactly 2 arguments: (reset! cell value)");
      if (args[0].type !== "atom") throw new Error("reset!: first argument must be a symbol");
      return Reset(args[0], args[1]);
    case "set!":
      if (args.length !== 2) throw new Error("set! requires exactly 2 arguments: (set! target:prop value)");
      if (args[0].type !== "atom" || !args[0].value.includes(":"))
        throw new Error("set! requires a property path (e.g., obj:prop), not a bare binding. Use (bind x val) for new bindings, (reset! cell val) for cells.");
      return SetProp(args[0], args[1]);
    case "set-symbol!":
      if (args.length !== 3) throw new Error("set-symbol! requires exactly 3 arguments: (set-symbol! obj key value)");
      return SetSymbol(args[0], args[1], args[2]);
    case "conj":
      if (args.length !== 2) throw new Error("conj requires exactly 2 arguments: (conj array value)");
      return Conj(args[0], args[1]);
    case "assoc": {
      if (args.length < 3) throw new Error("assoc requires at least 3 arguments: (assoc obj :key value)");
      const pairs = [];
      for (let i = 1; i < args.length; i += 2) {
        if (args[i].type !== "keyword") throw new Error(`assoc: expected keyword at position ${i}, got ${args[i]?.type ?? "nothing"}`);
        if (i + 1 >= args.length) throw new Error(`assoc: keyword :${args[i].value} has no value`);
        pairs.push({ key: args[i].value, value: args[i + 1] });
      }
      return Assoc(args[0], pairs);
    }
    case "dissoc": {
      if (args.length < 2) throw new Error("dissoc requires at least 2 arguments: (dissoc obj :key)");
      const keys = [];
      for (let i = 1; i < args.length; i++) {
        if (args[i].type !== "keyword") throw new Error(`dissoc: expected keyword at position ${i}, got ${args[i]?.type ?? "nothing"}`);
        keys.push(args[i].value);
      }
      return Dissoc(args[0], keys);
    }
    case "->":
      if (args.length < 2) throw new Error("-> requires at least 2 arguments: (-> value step...)");
      return Thread("first", args[0], args.slice(1));
    case "->>":
      if (args.length < 2) throw new Error("->> requires at least 2 arguments: (->> value step...)");
      return Thread("last", args[0], args.slice(1));
    case "some->":
      if (args.length < 2) throw new Error("some-> requires at least 2 arguments");
      return SomeThread("first", args[0], args.slice(1));
    case "some->>":
      if (args.length < 2) throw new Error("some->> requires at least 2 arguments");
      return SomeThread("last", args[0], args.slice(1));
    case "if-let": {
      if (args.length < 2 || args.length > 3) throw new Error("if-let requires 2-3 arguments: (if-let (binding expr) then else?)");
      const bp = args[0];
      if (!bp || bp.type !== "list" || bp.values.length !== 2) throw new Error("if-let: first argument must be (pattern expr)");
      return IfLet(bp, args[1], args.length === 3 ? args[2] : null);
    }
    case "when-let": {
      if (args.length < 2) throw new Error("when-let requires at least 2 arguments: (when-let (binding expr) body...)");
      const bp = args[0];
      if (!bp || bp.type !== "list" || bp.values.length !== 2) throw new Error("when-let: first argument must be (pattern expr)");
      return WhenLet(bp, args.slice(1));
    }
    default:
      return null;
  }
}

/**
 * Emit a typed AST node to kernel form.
 * @param {object} node - the typed AST node from classifySurfaceForm
 * @param {Function} sym - symbol constructor
 * @param {Function} array - array constructor
 * @returns {*} kernel S-expression
 */
function isKw(x) { return x && x.type === "keyword"; }
function isArr(x) { return x && x.type === "list" && Array.isArray(x.values); }

export function emitSurfaceForm(node, h) {
  const { sym, array, gensym } = h;
  switch (node.type) {
    case "Not":
      return array(sym("!"), node.operand);
    case "Swap": {
      const cellValue = sym(`${node.cell.value}:value`);
      return array(sym("="), cellValue, array(node.fn, cellValue, ...node.extraArgs));
    }
    case "Reset":
      return array(sym("="), sym(`${node.cell.value}:value`), node.value);
    case "SetProp":
      return array(sym("="), node.target, node.value);
    case "SetSymbol":
      return array(sym("="), array(sym("get"), node.obj, node.key), node.value);
    case "Conj":
      return array(sym("array"), array(sym("spread"), node.arr), node.item);
    case "Assoc": {
      const pairs = node.pairs.map(p => array(sym(p.key), p.value));
      return array(sym("object"), array(sym("spread"), node.obj), ...pairs);
    }
    case "Dissoc": {
      const aliasPatterns = node.keys.map(k => array(sym("alias"), sym(k), gensym("_")));
      const restVar = gensym("rest");
      const pattern = array(sym("object"), ...aliasPatterns, array(sym("rest"), restVar));
      const binding = array(sym("const"), pattern, node.obj);
      const arrowBody = array(sym("=>"), array(), binding, array(sym("return"), restVar));
      return array(arrowBody);
    }
    case "Thread": {
      let threaded = node.initial;
      for (const step of node.steps) {
        if (isKw(step)) {
          threaded = array(sym("."), threaded, sym(step.value));
        } else if (isArr(step) && step.values.length > 0 && isKw(step.values[0])) {
          const [kw, ...rest] = step.values;
          threaded = array(sym("."), threaded, sym(kw.value), ...rest);
        } else if (isArr(step)) {
          if (node.position === "first") {
            const [fn, ...rest] = step.values;
            threaded = array(fn, threaded, ...rest);
          } else {
            threaded = array(...step.values, threaded);
          }
        } else {
          threaded = array(step, threaded);
        }
      }
      return threaded;
    }
    case "SomeThread": {
      const stmts = [];
      let prevVar = gensym("t");
      stmts.push(array(sym("const"), prevVar, node.initial));
      stmts.push(array(sym("if"), array(sym("=="), prevVar, sym("null")), array(sym("return"), prevVar)));
      for (let i = 0; i < node.steps.length; i++) {
        const step = node.steps[i];
        let callExpr;
        if (isKw(step)) {
          callExpr = array(sym("."), prevVar, sym(step.value));
        } else if (isArr(step) && step.values.length > 0 && isKw(step.values[0])) {
          const [kw, ...rest] = step.values;
          callExpr = array(sym("."), prevVar, sym(kw.value), ...rest);
        } else if (isArr(step)) {
          if (node.position === "first") {
            const [fn, ...rest] = step.values;
            callExpr = array(fn, prevVar, ...rest);
          } else {
            callExpr = array(...step.values, prevVar);
          }
        } else {
          callExpr = array(step, prevVar);
        }
        if (i === node.steps.length - 1) {
          stmts.push(array(sym("return"), callExpr));
        } else {
          const nextVar = gensym("t");
          stmts.push(array(sym("const"), nextVar, callExpr));
          stmts.push(array(sym("if"), array(sym("=="), nextVar, sym("null")), array(sym("return"), nextVar)));
          prevVar = nextVar;
        }
      }
      const arrowFn = array(sym("=>"), array(), ...stmts);
      return array(arrowFn);
    }
    case "IfLet": {
      const pattern = node.bindingPair.values[0];
      const expr = node.bindingPair.values[1];
      const tempVar = gensym("t");
      const stmts = [array(sym("const"), tempVar, expr)];
      const result = compileLetPattern(pattern, tempVar);
      if (!result) throw new Error(`if-let: unrecognized pattern: ${formatSExpr(pattern)}`);
      const { condition, bindings } = result;
      const thenBlock = [...bindings, array(sym("return"), node.thenBody)];
      if (node.elseBody) {
        stmts.push(array(sym("if"), condition, array(sym("block"), ...thenBlock), array(sym("block"), array(sym("return"), node.elseBody))));
      } else {
        stmts.push(array(sym("if"), condition, array(sym("block"), ...thenBlock)));
      }
      return array(array(sym("=>"), array(), ...stmts));
    }
    case "WhenLet": {
      const pattern = node.bindingPair.values[0];
      const expr = node.bindingPair.values[1];
      const tempVar = gensym("t");
      const stmts = [array(sym("const"), tempVar, expr)];
      const result = compileLetPattern(pattern, tempVar);
      if (!result) throw new Error(`when-let: unrecognized pattern: ${formatSExpr(pattern)}`);
      const { condition, bindings } = result;
      const wrapped = wrapReturnLast(node.bodyForms);
      const returnBody = wrapped.length === 1 ? wrapped[0] : array(sym("block"), ...wrapped);
      stmts.push(array(sym("if"), condition, array(sym("block"), ...bindings, returnBody)));
      return array(array(sym("=>"), array(), ...stmts));
    }
    default:
      throw new Error(`Unknown surface AST node type: ${node.type}`);
  }
}
