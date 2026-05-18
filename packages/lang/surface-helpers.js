// DD-37 M22-3b: shared helpers extracted from surface.js.
// Used by both surface.js (for forms not yet migrated) and
// classifier.js (for forms migrated to the new architecture).

import {
  sym,
  array,
  gensym,
  isKeyword,
  isArray,
  formatSExpr,
} from "./expander.js";
import { toJsIdentifier } from "./compiler.js";

// Re-export expander helpers for classifier.js convenience
export { sym, array, gensym, isKeyword, isArray, formatSExpr, toJsIdentifier };

const STATEMENT_ONLY_HEADS = [
  "while", "for", "for-of", "for-in", "do-while", "switch",
  "label", "debugger",
  "block", "try", "catch", "finally",
  "var", "const", "let",
  "func", "fn", "class", "type", "export", "import",
];

export function isStatementOnlyForm(expr) {
  if (!isArray(expr) || expr.values.length === 0) return false;
  const head = expr.values[0];
  if (!head || head.type !== "atom") return false;
  const name = head.value;
  if (name === "if") return expr.values.length < 4;
  return STATEMENT_ONLY_HEADS.includes(name);
}

export function wrapReturnLast(bodyForms) {
  if (bodyForms.length === 0) return [];
  const lastExpr = bodyForms[bodyForms.length - 1];
  if (isStatementOnlyForm(lastExpr)) {
    return [...bodyForms];
  }
  if (bodyForms.length === 1) return [array(sym("return"), bodyForms[0])];
  return [...bodyForms.slice(0, -1), array(sym("return"), lastExpr)];
}

export function kernelArray(...items) {
  const node = array(...items);
  node._kernel = true;
  return node;
}

// Re-exported from surface.js for classifier.js use.
export { isPascalCase, compilePattern, andChain, getLiteralType, typeMatchesLiteral, buildTypeCheck, compileLetPattern, parseTypedParams, paramNameNodes, paramTypeChecks } from "./surface.js";
