import { read } from './reader.js';
import { expand } from './expander.js';
import { compile } from './compiler.js';

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

/**
 * Process all <script type="text/lykn"> elements in document order.
 */
async function processScripts() {
  const scripts = document.querySelectorAll('script[type="text/lykn"]');

  for (const el of scripts) {
    const label = el.src || 'inline script';
    try {
      if (el.src) {
        await load(el.src);
      } else {
        run(el.textContent);
      }
    } catch (err) {
      console.error(`[lykn] Error in ${label}:`, err);
    }
  }
}

// Auto-run on DOMContentLoaded (only in browser environment)
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', processScripts);
  } else {
    processScripts();
  }
}

// Expose public API
export { compileLykn as compile };
