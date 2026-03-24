// lykn s-expression reader
// Parses lykn source text into a simple AST:
//   { type: 'list', values: [...] }
//   { type: 'atom', value: 'identifier' }
//   { type: 'string', value: 'hello' }
//   { type: 'number', value: 42 }

export function read(source) {
  let pos = 0;

  function peek() {
    return source[pos];
  }

  function advance() {
    return source[pos++];
  }

  function skipWhitespaceAndComments() {
    while (pos < source.length) {
      const ch = peek();
      if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
        advance();
      } else if (ch === ';') {
        while (pos < source.length && peek() !== '\n') advance();
      } else {
        break;
      }
    }
  }

  function readString() {
    advance(); // skip opening "
    let value = '';
    while (pos < source.length && peek() !== '"') {
      if (peek() === '\\') {
        advance();
        const esc = advance();
        if (esc === 'n') value += '\n';
        else if (esc === 't') value += '\t';
        else if (esc === '\\') value += '\\';
        else if (esc === '"') value += '"';
        else value += esc;
      } else {
        value += advance();
      }
    }
    if (pos < source.length) advance(); // skip closing "
    return { type: 'string', value };
  }

  function readAtomOrNumber() {
    let value = '';
    while (pos < source.length) {
      const ch = peek();
      if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r' ||
          ch === '(' || ch === ')' || ch === ';') {
        break;
      }
      value += advance();
    }

    // Is it a number?
    if (/^-?\d+(\.\d+)?$/.test(value)) {
      return { type: 'number', value: parseFloat(value) };
    }

    return { type: 'atom', value };
  }

  function readList() {
    advance(); // skip (
    const values = [];
    skipWhitespaceAndComments();
    while (pos < source.length && peek() !== ')') {
      values.push(readExpr());
      skipWhitespaceAndComments();
    }
    if (pos < source.length) advance(); // skip )
    return { type: 'list', values };
  }

  function readExpr() {
    skipWhitespaceAndComments();
    if (pos >= source.length) return null;
    const ch = peek();
    if (ch === '(') return readList();
    if (ch === '"') return readString();
    return readAtomOrNumber();
  }

  // Read all top-level expressions
  const exprs = [];
  skipWhitespaceAndComments();
  while (pos < source.length) {
    const expr = readExpr();
    if (expr) exprs.push(expr);
    skipWhitespaceAndComments();
  }
  return exprs;
}
