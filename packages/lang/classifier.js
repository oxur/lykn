// DD-37 JS-side classifier (Phase 0 pilot).
// Pass-through wrapper around existing surface dispatch.
// Currently only handles `not`; other forms pass through to the
// existing surface macro path in expander.js/surface.js.

import { Not, Swap, Reset, SetProp, SetSymbol, Conj, Assoc, Dissoc } from "./surface-ast.js";

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
    default:
      throw new Error(`Unknown surface AST node type: ${node.type}`);
  }
}
