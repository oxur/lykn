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
