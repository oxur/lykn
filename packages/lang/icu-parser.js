// ICU MessageFormat subset parser for lykn template macro (DD-54 Phase A).
// Hand-written recursive descent; no runtime dependencies.
//
// Parses an ICU string into a Message Format Tree (MFT):
//   - { type: "literal", value: string }
//   - { type: "slot", name: string }
//   - { type: "plural", name: string, branches: PluralBranch[], offset?: number }
//   - { type: "select", name: string, branches: SelectBranch[] }
//
// PluralBranch: { key: string, body: MFTNode[] }
//   key is "=N" (exact), or a CLDR category (zero|one|two|few|many|other)
// SelectBranch: { key: string, body: MFTNode[] }

const ALL_CLDR_CATEGORIES = new Set(["zero", "one", "two", "few", "many", "other"]);
const ENGLISH_CLDR_CATEGORIES = new Set(["one", "other"]);

export function parseIcu(input) {
  const parser = new IcuParser(input);
  const nodes = parser.parseMessage();
  if (parser.pos < input.length) {
    parser.error(`unexpected character '${input[parser.pos]}'`);
  }
  return nodes;
}

export function collectSlotNames(nodes) {
  const names = new Set();
  for (const node of nodes) {
    collectSlotNamesInner(node, names);
  }
  return names;
}

function collectSlotNamesInner(node, names) {
  if (node.type === "slot") {
    names.add(node.name);
  } else if (node.type === "plural" || node.type === "select") {
    names.add(node.name);
    for (const branch of node.branches) {
      for (const child of branch.body) {
        collectSlotNamesInner(child, names);
      }
    }
  }
}

class IcuParser {
  constructor(input) {
    this.input = input;
    this.pos = 0;
  }

  error(msg) {
    throw new IcuParseError(msg, this.input, this.pos);
  }

  peek() {
    return this.pos < this.input.length ? this.input[this.pos] : null;
  }

  advance() {
    return this.input[this.pos++];
  }

  expect(ch) {
    if (this.peek() !== ch) {
      const got = this.peek() ?? "end of input";
      this.error(`expected '${ch}', got '${got}'`);
    }
    this.advance();
  }

  skipWhitespace() {
    while (this.pos < this.input.length && /\s/.test(this.input[this.pos])) {
      this.pos++;
    }
  }

  // Parse a top-level message (sequence of literal/slot/plural/select nodes).
  // Stops at end of input or at an unmatched '}'.
  parseMessage() {
    const nodes = [];
    while (this.pos < this.input.length) {
      const ch = this.peek();
      if (ch === "}") break;
      if (ch === "{") {
        nodes.push(this.parseBlock());
      } else if (ch === "'") {
        nodes.push(this.parseEscape());
      } else {
        nodes.push(this.parseLiteral());
      }
    }
    return coalesceLiterals(nodes);
  }

  // Parse a branch body (inside { ... } of a plural/select branch).
  // Same as parseMessage but stops at '}'.
  parseBranchBody() {
    const nodes = [];
    while (this.pos < this.input.length && this.peek() !== "}") {
      const ch = this.peek();
      if (ch === "{") {
        nodes.push(this.parseBlock());
      } else if (ch === "'") {
        nodes.push(this.parseEscape());
      } else if (ch === "#") {
        nodes.push({ type: "octothorpe" });
        this.advance();
      } else {
        nodes.push(this.parseBranchLiteral());
      }
    }
    return coalesceLiterals(nodes);
  }

  parseLiteral() {
    let value = "";
    while (this.pos < this.input.length) {
      const ch = this.peek();
      if (ch === "{" || ch === "}" || ch === "'") break;
      value += this.advance();
    }
    return { type: "literal", value };
  }

  parseBranchLiteral() {
    let value = "";
    while (this.pos < this.input.length) {
      const ch = this.peek();
      if (ch === "{" || ch === "}" || ch === "'" || ch === "#") break;
      value += this.advance();
    }
    return { type: "literal", value };
  }

  parseEscape() {
    this.advance(); // consume opening '
    const next = this.peek();
    if (next === "{" || next === "}" || next === "'") {
      if (next === "'") {
        this.advance();
        return { type: "literal", value: "'" };
      }
      const ch = this.advance();
      // In ICU, '{' and '}' are quoted by surrounding with single quotes: '{' or '}'
      // Consume until closing quote
      if (this.peek() === "'") {
        this.advance();
      }
      return { type: "literal", value: ch };
    }
    // A lone quote not followed by { } or ' is a literal quote
    return { type: "literal", value: "'" };
  }

  // Parse { ... } — could be a simple slot, plural, or select.
  parseBlock() {
    const startPos = this.pos;
    this.expect("{");
    this.skipWhitespace();

    const name = this.parseIdentifier();
    if (!name) {
      this.error("expected slot name after '{'");
    }
    this.skipWhitespace();

    if (this.peek() === "}") {
      this.advance();
      return { type: "slot", name };
    }

    if (this.peek() === ",") {
      this.advance();
      this.skipWhitespace();
      const kind = this.parseIdentifier();
      this.skipWhitespace();

      if (kind === "plural") {
        return this.parsePluralBody(name);
      } else if (kind === "select") {
        return this.parseSelectBody(name);
      } else {
        this.error(`unknown format type '${kind}'; expected 'plural' or 'select'`);
      }
    }

    this.error(`expected '}' or ',' after slot name '${name}'`);
  }

  parseIdentifier() {
    let id = "";
    while (this.pos < this.input.length) {
      const ch = this.input[this.pos];
      if (/[a-zA-Z0-9_-]/.test(ch)) {
        id += ch;
        this.pos++;
      } else {
        break;
      }
    }
    return id;
  }

  parsePluralBody(name) {
    this.expect(",");
    this.skipWhitespace();

    const branches = [];
    let hasOther = false;

    while (this.pos < this.input.length && this.peek() !== "}") {
      this.skipWhitespace();
      if (this.peek() === "}") break;

      const key = this.parsePluralKey();
      this.skipWhitespace();

      if (key === "other") hasOther = true;

      this.expect("{");
      const body = this.parseBranchBody();
      this.expect("}");

      // Resolve # inside this branch to the selector name
      const resolved = resolveOctothorpe(body, name);
      branches.push({ key, body: resolved });
      this.skipWhitespace();
    }

    if (!hasOther) {
      this.error(`plural block for {${name}} missing required 'other' branch`);
    }

    // English CLDR Phase A: =1 collides with `one`
    const exactValues = new Set();
    const categoryKeys = new Set();
    for (const b of branches) {
      if (b.key.startsWith("=")) {
        exactValues.add(parseInt(b.key.slice(1), 10));
      } else {
        categoryKeys.add(b.key);
      }
    }
    if (exactValues.has(1) && categoryKeys.has("one")) {
      this.error(
        `plural block for {${name}} has overlapping branches: ` +
        `'=1' and 'one' both match count == 1 under English plural rules. ` +
        `Remove one — they handle the same case.`
      );
    }

    this.expect("}");
    return { type: "plural", name, branches };
  }

  parsePluralKey() {
    if (this.peek() === "=") {
      this.advance();
      let num = "";
      while (this.pos < this.input.length && /[0-9]/.test(this.input[this.pos])) {
        num += this.advance();
      }
      if (!num) this.error("expected number after '=' in plural key");
      return `=${num}`;
    }
    const cat = this.parseIdentifier();
    if (!cat) this.error("expected plural category or '=N'");
    if (!ALL_CLDR_CATEGORIES.has(cat)) {
      this.error(
        `unknown plural category '${cat}'; ` +
        `valid CLDR categories: ${[...ALL_CLDR_CATEGORIES].join(" ")}`
      );
    }
    if (ALL_CLDR_CATEGORIES.has(cat) && !ENGLISH_CLDR_CATEGORIES.has(cat)) {
      this.error(
        `plural category '${cat}' is not valid under English plural rules. ` +
        `English CLDR uses only 'one' and 'other'. ` +
        `Hint: use '=N {...}' for specific numeric values, ` +
        `e.g. '=0 {none}' for n=0 or '=2 {pair}' for n=2.`
      );
    }
    return cat;
  }

  parseSelectBody(name) {
    this.expect(",");
    this.skipWhitespace();

    const branches = [];
    let hasOther = false;

    while (this.pos < this.input.length && this.peek() !== "}") {
      this.skipWhitespace();
      if (this.peek() === "}") break;

      const key = this.parseIdentifier();
      if (!key) this.error("expected select branch key");
      this.skipWhitespace();

      if (key === "other") hasOther = true;

      this.expect("{");
      const body = this.parseBranchBody();
      this.expect("}");

      branches.push({ key, body });
      this.skipWhitespace();
    }

    if (!hasOther) {
      this.error(`select block for {${name}} missing required 'other' branch`);
    }

    this.expect("}");
    return { type: "select", name, branches };
  }
}

export class IcuParseError extends Error {
  constructor(msg, input, pos) {
    super(`${msg}\n  in "${input}"\n  at position ${pos}`);
    this.name = "IcuParseError";
    this.input = input;
    this.position = pos;
  }
}

function resolveOctothorpe(nodes, selectorName) {
  return nodes.map((node) => {
    if (node.type === "octothorpe") {
      return { type: "slot", name: selectorName };
    }
    if (node.type === "plural" || node.type === "select") {
      return {
        ...node,
        branches: node.branches.map((b) => ({
          ...b,
          body: resolveOctothorpe(b.body, selectorName),
        })),
      };
    }
    return node;
  });
}

function coalesceLiterals(nodes) {
  const result = [];
  for (const node of nodes) {
    if (
      node.type === "literal" &&
      result.length > 0 &&
      result[result.length - 1].type === "literal"
    ) {
      result[result.length - 1].value += node.value;
    } else {
      result.push(node);
    }
  }
  return result;
}
