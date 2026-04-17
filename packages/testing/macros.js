// Testing DSL surface macros (DD-30)
// Loaded via (surface-macros "macros.js") in mod.lykn.
// Parameters provided by the expander: macroEnv, sym, array, gensym,
//   isArray, isSymbol, isNumber, isString, isKeyword,
//   first, rest, nth, length, append, formatSExpr

// --- helpers ---

function hasAwait(node) {
  if (!node) return false;
  if (node.type === 'atom' && node.value === 'await') return true;
  if (node.type === 'list' && node.values) {
    return node.values.some(hasAwait);
  }
  return false;
}

function hasStepChild(forms) {
  return forms.some(f =>
    f.type === 'list' && f.values.length > 0 &&
    f.values[0].type === 'atom' && f.values[0].value === 'step'
  );
}

function hasKeywords(args) {
  return args.some(a => a.type === 'keyword' &&
    (a.value === 'setup' || a.value === 'teardown' || a.value === 'body'));
}

function parseKeywordClauses(args) {
  const setup = [];
  const teardown = [];
  const body = [];
  let current = 'body';

  for (const a of args) {
    if (a.type === 'keyword' && a.value === 'setup') { current = 'setup'; continue; }
    if (a.type === 'keyword' && a.value === 'teardown') { current = 'teardown'; continue; }
    if (a.type === 'keyword' && a.value === 'body') { current = 'body'; continue; }
    if (current === 'setup') setup.push(a);
    else if (current === 'teardown') teardown.push(a);
    else body.push(a);
  }
  return { setup, teardown, body };
}

function buildTestBody(setup, teardown, body) {
  const allBody = [...setup, ...body];
  if (teardown.length === 0) return allBody;
  const tryBlock = array(sym('block'), ...allBody);
  const finallyBlock = array(sym('block'), ...teardown);
  return [array(sym('try'), tryBlock, array(sym('finally'), finallyBlock))];
}

// --- assertion macros ---

// (is expr) → (assert expr)
macroEnv.set('is', (expr) =>
  array(sym('assert'), expr));

// (is-equal actual expected) → (assert-equals actual expected)
macroEnv.set('is-equal', (actual, expected) =>
  array(sym('assert-equals'), actual, expected));

// (is-not-equal actual expected) → (assert-not-equals actual expected)
macroEnv.set('is-not-equal', (actual, expected) =>
  array(sym('assert-not-equals'), actual, expected));

// (is-strict-equal actual expected) → (assert-strict-equals actual expected)
macroEnv.set('is-strict-equal', (actual, expected) =>
  array(sym('assert-strict-equals'), actual, expected));

// (ok expr) → (assert-exists expr)
macroEnv.set('ok', (expr) =>
  array(sym('assert-exists'), expr));

// (is-thrown body) / (is-thrown body ErrType) / (is-thrown body ErrType "msg")
macroEnv.set('is-thrown', (...args) => {
  const body = args[0];
  const fn = array(sym('=>'), array(), body);
  if (args[2]) return array(sym('assert-throws'), fn, args[1], args[2]);
  if (args[1]) return array(sym('assert-throws'), fn, args[1]);
  return array(sym('assert-throws'), fn);
});

// (is-thrown-async body) / (is-thrown-async body ErrType)
macroEnv.set('is-thrown-async', (...args) => {
  const body = args[0];
  const fn = array(sym('async'), array(sym('=>'), array(), body));
  if (args[1]) return array(sym('await'), array(sym('assert-rejects'), fn, args[1]));
  return array(sym('await'), array(sym('assert-rejects'), fn));
});

// (matches str pattern) → (assert-match str pattern)
macroEnv.set('matches', (str, pattern) =>
  array(sym('assert-match'), str, pattern));

// (includes str substr) → (assert-string-includes str substr)
macroEnv.set('includes', (str, substr) =>
  array(sym('assert-string-includes'), str, substr));

// (has arr items) → (assert-array-includes arr items)
macroEnv.set('has', (arr, items) =>
  array(sym('assert-array-includes'), arr, items));

// (obj-matches actual expected) → (assert-object-match actual expected)
macroEnv.set('obj-matches', (actual, expected) =>
  array(sym('assert-object-match'), actual, expected));

// --- test definition macros ---

// (test "name" body...) or (test "name" :setup expr :teardown expr :body body...)
macroEnv.set('test', (name, ...args) => {
  let setup, teardown, body;
  if (hasKeywords(args)) {
    ({ setup, teardown, body } = parseKeywordClauses(args));
  } else {
    setup = []; teardown = []; body = args;
  }

  const allForms = [...setup, ...body];
  const needsStep = hasStepChild(body);
  const needsAsync = needsStep || hasAwait(array(sym('_'), ...allForms));
  const fnBody = buildTestBody(setup, teardown, body);

  if (needsStep) {
    return array(sym('.'), sym('Deno'), sym('test'), name,
      array(sym('async'), array(sym('=>'), array(sym('t')), ...fnBody)));
  }
  if (needsAsync) {
    return array(sym('.'), sym('Deno'), sym('test'), name,
      array(sym('async'), array(sym('=>'), array(), ...fnBody)));
  }
  return array(sym('.'), sym('Deno'), sym('test'), name,
    array(sym('=>'), array(), ...fnBody));
});

// (test-async "name" body...) — always async
macroEnv.set('test-async', (name, ...args) => {
  let setup, teardown, body;
  if (hasKeywords(args)) {
    ({ setup, teardown, body } = parseKeywordClauses(args));
  } else {
    setup = []; teardown = []; body = args;
  }

  const needsStep = hasStepChild(body);
  const fnBody = buildTestBody(setup, teardown, body);

  if (needsStep) {
    return array(sym('.'), sym('Deno'), sym('test'), name,
      array(sym('async'), array(sym('=>'), array(sym('t')), ...fnBody)));
  }
  return array(sym('.'), sym('Deno'), sym('test'), name,
    array(sym('async'), array(sym('=>'), array(), ...fnBody)));
});

// (suite "name" :setup expr :teardown expr (test ...) (test ...) ...)
// Child (test "name" body...) forms are converted to (step "name" body...)
macroEnv.set('suite', (name, ...args) => {
  let setup, teardown, children;
  if (hasKeywords(args)) {
    ({ setup, teardown, body: children } = parseKeywordClauses(args));
  } else {
    setup = []; teardown = []; children = args;
  }

  // Convert child (test ...) into (step ...) so they compile to t.step()
  const converted = children.map(child => {
    if (child.type === 'list' && child.values.length > 0 &&
        child.values[0].type === 'atom' && child.values[0].value === 'test') {
      return array(sym('step'), ...child.values.slice(1));
    }
    return child;
  });

  const fnBody = buildTestBody(setup, teardown, converted);
  return array(sym('.'), sym('Deno'), sym('test'), name,
    array(sym('async'), array(sym('=>'), array(sym('t')), ...fnBody)));
});

// (step "name" body...) → (await (. t step "name" (=> () body...)))
macroEnv.set('step', (name, ...body) => {
  const needsAsync = hasAwait(array(sym('_'), ...body));
  const stepFn = needsAsync
    ? array(sym('async'), array(sym('=>'), array(), ...body))
    : array(sym('=>'), array(), ...body);
  return array(sym('await'),
    array(sym('.'), sym('t'), sym('step'), name, stepFn));
});

// --- convenience macros ---

// (test-compiles "name" input expected)
// Expands to: (test "name" (bind r#gen (compile input)) (is-equal (r#gen:trim) expected))
macroEnv.set('test-compiles', (name, input, expected) => {
  const tmp = gensym('r');
  const trimCall = sym(tmp.value + ':trim');
  return array(sym('test'), name,
    array(sym('bind'), tmp, array(sym('compile'), input)),
    array(sym('is-equal'), array(trimCall), expected));
});
