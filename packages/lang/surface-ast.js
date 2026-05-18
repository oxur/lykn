// DD-37 surface AST node constructors.
// Each surface form that migrates out of surface.js gets a typed AST node here.
// Initially only what the `not` pilot needs (M21).

/**
 * AST node for the `not` surface form: (not expr) → (! expr).
 * @param {*} operand - the S-expression operand
 * @returns {{ type: "Not", operand: * }}
 */
export function Not(operand) {
  return { type: "Not", operand };
}

// Batch 1: mutation primitives
export function Swap(cell, fn, extraArgs) {
  return { type: "Swap", cell, fn, extraArgs };
}
export function Reset(cell, value) {
  return { type: "Reset", cell, value };
}
export function SetProp(target, value) {
  return { type: "SetProp", target, value };
}
export function SetSymbol(obj, key, value) {
  return { type: "SetSymbol", obj, key, value };
}
