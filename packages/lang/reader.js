/**
 * @module
 * lykn s-expression reader.
 * Parses lykn source text into an AST of five node types.
 *
 * @typedef {AtomNode | StringNode | NumberNode | ListNode | ConsNode} AstNode
 *
 * @typedef {Object} AtomNode
 * @property {'atom'} type
 * @property {string} value
 *
 * @typedef {Object} StringNode
 * @property {'string'} type
 * @property {string} value
 *
 * @typedef {Object} NumberNode
 * @property {'number'} type
 * @property {number} value
 * @property {number} [base] - Original radix (2–36) for #NNr literals
 *
 * @typedef {Object} ListNode
 * @property {'list'} type
 * @property {AstNode[]} values
 *
 * @typedef {Object} ConsNode
 * @property {'cons'} type
 * @property {AstNode} car - Element before the dot
 * @property {AstNode} cdr - Element after the dot
 */

/**
 * Parse lykn source text into an AST.
 * @param {string} source - lykn source code
 * @returns {AstNode[]} Array of top-level AST nodes
 * @throws {Error} For malformed input
 */
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
          ch === '(' || ch === ')' || ch === ';' ||
          ch === '`' || ch === ',' || ch === "'") {
        break;
      }
      value += advance();
    }

    if (/^-?\d+(\.\d+)?$/.test(value)) {
      return { type: 'number', value: parseFloat(value) };
    }

    if (value.length > 1 && value.startsWith(':')) {
      return { type: 'keyword', value: value.slice(1) };
    }

    return { type: 'atom', value };
  }

  function readList() {
    advance(); // skip (
    const values = [];
    let sawDot = false;
    let cdrNode = null;

    skipWhitespaceAndComments();
    while (pos < source.length && peek() !== ')') {
      const expr = readExpr();
      if (expr === null) {
        skipWhitespaceAndComments();
        continue;
      }

      if (expr.type === 'atom' && expr.value === '.') {
        if (values.length === 0) {
          throw new Error('dot cannot be first element in list');
        }
        if (sawDot) {
          throw new Error('only one dot allowed per list level');
        }
        sawDot = true;

        skipWhitespaceAndComments();
        if (pos >= source.length || peek() === ')') {
          throw new Error('nothing after dot in list');
        }
        cdrNode = readExpr();
        if (cdrNode === null) {
          throw new Error('nothing after dot in list');
        }

        skipWhitespaceAndComments();
        if (pos < source.length && peek() !== ')') {
          throw new Error('only one element allowed after dot in list');
        }
      } else {
        if (sawDot) {
          throw new Error('only one element allowed after dot in list');
        }
        values.push(expr);
      }
      skipWhitespaceAndComments();
    }
    if (pos < source.length) advance(); // skip )

    if (sawDot) {
      if (values.length !== 1) {
        throw new Error('dotted pair must have exactly one element before the dot');
      }
      return { type: 'cons', car: values[0], cdr: cdrNode };
    }

    return { type: 'list', values };
  }

  function readDispatch() {
    advance(); // consume '#'
    if (pos >= source.length) {
      throw new Error('unexpected end of input after #');
    }
    const ch = peek();

    if (ch === ';') return readExprComment();
    if (ch === '|') return readBlockComment();
    if (ch === 'a') return readDataLiteral('array');
    if (ch === 'o') return readDataLiteral('object');
    if (ch === '(') {
      throw new Error('use #a(...) for array literals');
    }
    if (ch >= '0' && ch <= '9') return readRadixLiteral();

    throw new Error(`unknown dispatch character: ${ch}`);
  }

  function readExprComment() {
    advance(); // consume ';'
    skipWhitespaceAndComments();
    if (pos >= source.length) {
      throw new Error('#; at end of input with no form to discard');
    }
    if (peek() === ')') {
      throw new Error('#; at end of list with no form to discard');
    }
    readExpr(); // read and discard
    return null;
  }

  function readBlockComment() {
    advance(); // consume '|'
    let depth = 1;

    while (pos < source.length && depth > 0) {
      const ch = advance();
      if (ch === '#' && pos < source.length && peek() === '|') {
        advance();
        depth++;
      } else if (ch === '|' && pos < source.length && peek() === '#') {
        advance();
        depth--;
      }
    }

    if (depth > 0) {
      throw new Error('unterminated block comment (missing |#)');
    }
    return null;
  }

  function readDataLiteral(formName) {
    advance(); // consume the letter ('a' or 'o')
    skipWhitespaceAndComments();
    if (pos >= source.length || peek() !== '(') {
      throw new Error(`#${formName[0]} must be followed by (...)`);
    }
    const list = readList();
    return {
      type: 'list',
      values: [{ type: 'atom', value: formName }, ...list.values],
    };
  }

  function readRadixLiteral() {
    let baseStr = '';
    while (pos < source.length && peek() !== 'r') {
      const ch = peek();
      if (ch >= '0' && ch <= '9') {
        baseStr += advance();
      } else {
        throw new Error(`expected 'r' after base in radix literal, got '${ch}'`);
      }
    }

    if (pos >= source.length) {
      throw new Error("expected 'r' after base in radix literal, got end of input");
    }

    advance(); // consume 'r'

    const base = parseInt(baseStr, 10);
    if (base < 2 || base > 36) {
      throw new Error(`radix base must be 2-36, got ${base}`);
    }

    let valueStr = '';
    while (pos < source.length) {
      const ch = peek();
      if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r' ||
          ch === '(' || ch === ')' || ch === ';' ||
          ch === '`' || ch === ',' || ch === "'") {
        break;
      }
      valueStr += advance();
    }

    if (valueStr === '') {
      throw new Error(`missing value after #${baseStr}r`);
    }

    for (const digit of valueStr) {
      const digitVal = parseInt(digit, 36);
      if (Number.isNaN(digitVal) || digitVal >= base) {
        throw new Error(`'${digit}' is not a valid digit in base ${base}`);
      }
    }

    const value = parseInt(valueStr, base);
    return { type: 'number', value, base };
  }

  function readExpr() {
    skipWhitespaceAndComments();
    if (pos >= source.length) return null;
    const ch = peek();
    if (ch === ')') return null;
    if (ch === '(') return readList();
    if (ch === '"') return readString();

    if (ch === '`') {
      advance();
      const expr = readExpr();
      return { type: 'list', values: [{ type: 'atom', value: 'quasiquote' }, expr] };
    }
    if (ch === "'") {
      advance();
      const expr = readExpr();
      return { type: 'list', values: [{ type: 'atom', value: 'quote' }, expr] };
    }
    if (ch === ',') {
      advance();
      if (peek() === '@') {
        advance();
        const expr = readExpr();
        return { type: 'list', values: [{ type: 'atom', value: 'unquote-splicing' }, expr] };
      }
      const expr = readExpr();
      return { type: 'list', values: [{ type: 'atom', value: 'unquote' }, expr] };
    }
    if (ch === '#') {
      const node = readDispatch();
      if (node === null) return readExpr();
      return node;
    }

    return readAtomOrNumber();
  }

  // Read all top-level expressions
  const exprs = [];
  skipWhitespaceAndComments();
  while (pos < source.length) {
    const expr = readExpr();
    if (expr !== null) exprs.push(expr);
    skipWhitespaceAndComments();
  }
  return exprs;
}
