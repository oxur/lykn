// lykn - s-expression syntax for JavaScript
// https://github.com/lykn

export { read } from './reader.js';
export { compile, compileExpr } from './compiler.js';

import { read } from './reader.js';
import { compile } from './compiler.js';

/**
 * Compile lykn source code to JavaScript.
 * @param {string} source - lykn source text
 * @returns {string} - JavaScript source text
 */
export function lykn(source) {
  return compile(read(source));
}
