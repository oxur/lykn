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

// Batch 5: anonymous functions
export function Fn(paramList, bodyForms) {
  return { type: "Fn", paramList, bodyForms };
}

// Batch 6: logical n-ary
export function And(args) { return { type: "And", args }; }
export function Or(args) { return { type: "Or", args }; }

// Batch 7: small surface forms
export function Express(cell) { return { type: "Express", cell }; }
export function Obj(pairs) { return { type: "Obj", pairs }; }
export function Cell(value) { return { type: "Cell", value }; }

// Batch 8+9: remaining forms
export function Bind(args) { return { type: "Bind", args }; }
export function Eq(args) { return { type: "Eq", args }; }
export function Neq(a, b) { return { type: "Neq", a, b }; }
export function Func(nameNode, restArgs) { return { type: "Func", nameNode, restArgs }; }
export function GenFunc(nameNode, restArgs) { return { type: "GenFunc", nameNode, restArgs }; }
export function GenFn(paramList, args) { return { type: "GenFn", paramList, args }; }
export function Match(expr, clauses) { return { type: "Match", expr, clauses }; }
export function TypeDef(typeName, constructors) { return { type: "TypeDef", typeName, constructors }; }
