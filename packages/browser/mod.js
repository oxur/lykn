/**
 * @lykn/browser — browser runtime for lykn
 *
 * Provides compile, run, and load functions, plus automatic
 * processing of <script type="text/lykn"> elements.
 *
 * Usage as a bundle:
 *   <script src="lykn-browser.js"></script>
 *   <script type="text/lykn">
 *     (console:log "Hello from lykn!")
 *   </script>
 *
 * Usage as an ES module:
 *   import { compile, run, load } from '@lykn/browser';
 */

export { compileLykn as compile, run, load } from './compiler.js';

// Import scripts module for its side effect (auto-processing DOM scripts)
import './scripts.js';
