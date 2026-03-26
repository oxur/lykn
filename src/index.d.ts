/** Parse lykn source text into an s-expression AST. */
export function read(source: string): object[];

/** Compile an array of s-expression AST nodes to a JavaScript string. */
export function compile(exprs: object[]): string;

/** Compile a single s-expression AST node to an ESTree node. */
export function compileExpr(node: object): object;

/** Compile lykn source code to JavaScript. */
export function lykn(source: string): string;
