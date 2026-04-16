import { read } from 'lang/reader.js';
import { expand } from 'lang/expander.js';
import { compile } from 'lang/compiler.js';

/**
 * Compile lykn source to JavaScript string.
 * @param {string} source - lykn source text
 * @returns {string} JavaScript source text
 */
export function compileLykn(source) {
  return compile(expand(read(source)));
}

/**
 * Compile and execute lykn source.
 * @param {string} source - lykn source text
 * @returns {*} result of eval
 */
export function run(source) {
  const js = compileLykn(source);
  return (0, eval)(js);
}

/**
 * Fetch a .lykn file, compile, and execute.
 * @param {string} url - URL to fetch
 * @returns {Promise<*>} result of eval
 */
export async function load(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`[lykn] Failed to load ${url}: ${response.status}`);
  }
  const source = await response.text();
  return run(source);
}
