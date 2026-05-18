// DD-37 JS-side classifier (Phase 0 pilot).
// Pass-through wrapper around existing surface dispatch.
// Currently only handles `not`; other forms pass through to the
// existing surface macro path in expander.js/surface.js.

import { Not } from "./surface-ast.js";

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
export function emitSurfaceForm(node, sym, array) {
  switch (node.type) {
    case "Not":
      return array(sym("!"), node.operand);
    default:
      throw new Error(`Unknown surface AST node type: ${node.type}`);
  }
}
