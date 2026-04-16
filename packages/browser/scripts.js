import { run, load } from './compiler.js';

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
