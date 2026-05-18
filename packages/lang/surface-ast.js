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

// Batch 2: collection ops
export function Conj(arr, item) {
  return { type: "Conj", arr, item };
}
export function Assoc(obj, pairs) {
  return { type: "Assoc", obj, pairs };
}
export function Dissoc(obj, keys) {
  return { type: "Dissoc", obj, keys };
}

// Batch 3: threading macros
export function Thread(position, initial, steps) {
  return { type: "Thread", position, initial, steps };
}
export function SomeThread(position, initial, steps) {
  return { type: "SomeThread", position, initial, steps };
}

// Batch 4: binding macros
export function IfLet(bindingPair, thenBody, elseBody) {
  return { type: "IfLet", bindingPair, thenBody, elseBody };
}
export function WhenLet(bindingPair, bodyForms) {
  return { type: "WhenLet", bindingPair, bodyForms };
}
