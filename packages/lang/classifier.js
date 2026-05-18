// DD-37 JS-side classifier (Phase 0 pilot).
// Pass-through wrapper around existing surface dispatch.
// Currently only handles `not`; other forms pass through to the
// existing surface macro path in expander.js/surface.js.

import { Not, Swap, Reset, SetProp, SetSymbol } from "./surface-ast.js";

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
    default:
      throw new Error(`Unknown surface AST node type: ${node.type}`);
  }
}
