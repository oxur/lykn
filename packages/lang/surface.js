/**
 * @module
 * lykn surface form macros.
 * Each macro transforms surface syntax to kernel forms (DD-01 through DD-09).
 * These are the JS reference implementation; the Rust compiler will produce
 * identical expansions as static transforms.
 */

import {
	sym,
	array,
	gensym,
	isKeyword,
	isArray,
	formatSExpr,
} from "./expander.js";

/**
 * Create a kernel-level AST node that won't be re-intercepted by surface
 * macros during fixed-point expansion. Used by surface macros (reset!,
 * swap!) that emit kernel `=` (assignment) — prevents the surface `=`
 * macro (equality) from re-expanding their output.
 */
function kernelArray(...items) {
	const node = array(...items);
	node._kernel = true;
	return node;
}

// --- Type Registry ---
// Maps constructor names to their field names, populated by `type` macro.
// Used by `match` and `if-let`/`when-let` to resolve ADT pattern field bindings.
const typeRegistry = new Map();

export function resetTypeRegistry() {
	typeRegistry.clear();
	// Pre-populate with blessed prelude types (DD-17)
	typeRegistry.set("Some", ["value"]);
	typeRegistry.set("None", []);
	typeRegistry.set("Ok", ["value"]);
	typeRegistry.set("Err", ["error"]);
}

// --- Shared Helpers ---

function isPascalCase(name) {
	return name.length > 0 && name[0] >= "A" && name[0] <= "Z";
}

/**
 * Determine the static type of a literal AST node, or null if not a literal.
 */
function getLiteralType(node) {
	if (node.type === "number") return "number";
	if (node.type === "string") return "string";
	if (node.type === "atom") {
		if (node.value === "true" || node.value === "false") return "boolean";
		if (node.value === "null") return "null";
		if (node.value === "undefined") return "undefined";
		if (node.value === "NaN") return "NaN";
	}
	// #a(...) array literal — classified as a list with head "array"
	if (
		node.type === "list" &&
		node.values.length > 0 &&
		node.values[0].type === "atom" &&
		node.values[0].value === "array"
	)
		return "array";
	// (obj ...) — classified as a list with head "obj"
	if (
		node.type === "list" &&
		node.values.length > 0 &&
		node.values[0].type === "atom" &&
		node.values[0].value === "obj"
	)
		return "object";
	return null;
}

/**
 * Check if a type annotation matches a statically known literal type.
 */
function typeMatchesLiteral(typeName, literalType) {
	if (literalType === "NaN") return false; // NaN fails :number
	if (literalType === "null") return false; // null fails :object
	if (literalType === "undefined") return false; // undefined fails everything
	return typeName === literalType;
}

/**
 * Build a type check assertion for a parameter.
 * Returns a kernel (if (check) (throw (new TypeError msg))) form, or null for :any.
 */
function buildTypeCheck(paramNode, typeKw, funcName, label) {
	const typeName = typeKw.value;
	if (typeName === "any") return null;

	const paramName = paramNode.value;
	const msgText = label
		? `${funcName}: ${label} '${paramName}' expected ${typeName}, got `
		: `${funcName} '${paramName}': expected ${typeName}, got `;
	const msg = {
		type: "string",
		value: msgText,
	};
	const typeofParam = array(sym("typeof"), paramNode);
	const errorMsg = array(sym("+"), msg, typeofParam);
	const throwStmt = array(
		sym("throw"),
		array(sym("new"), sym("TypeError"), errorMsg),
	);

	let check;
	switch (typeName) {
		case "number":
			check = array(
				sym("||"),
				array(sym("!=="), typeofParam, { type: "string", value: "number" }),
				array(sym("Number:isNaN"), paramNode),
			);
			break;
		case "string":
			check = array(sym("!=="), typeofParam, {
				type: "string",
				value: "string",
			});
			break;
		case "boolean":
			check = array(sym("!=="), typeofParam, {
				type: "string",
				value: "boolean",
			});
			break;
		case "function":
			check = array(sym("!=="), typeofParam, {
				type: "string",
				value: "function",
			});
			break;
		case "object":
			check = array(
				sym("||"),
				array(sym("!=="), typeofParam, { type: "string", value: "object" }),
				array(sym("==="), paramNode, sym("null")),
			);
			break;
		case "array":
			check = array(sym("!"), array(sym("Array:isArray"), paramNode));
			break;
		case "symbol":
			check = array(sym("!=="), typeofParam, {
				type: "string",
				value: "symbol",
			});
			break;
		case "bigint":
			check = array(sym("!=="), typeofParam, {
				type: "string",
				value: "bigint",
			});
			break;
		default:
			// User-defined type — check for tagged object
			check = array(
				sym("||"),
				array(sym("!=="), typeofParam, { type: "string", value: "object" }),
				array(
					sym("||"),
					array(sym("==="), paramNode, sym("null")),
					array(
						sym("!"),
						array(sym("in"), { type: "string", value: "tag" }, paramNode),
					),
				),
			);
			break;
	}

	return array(sym("if"), check, throwStmt);
}

/** Valid clause keys for func/fn keyword parsing. */
const FUNC_CLAUSE_KEYS = new Set(["args", "returns", "yields", "pre", "post", "body"]);

/**
 * Parse keyword-value pairs from an args list.
 * Only keywords in FUNC_CLAUSE_KEYS are treated as clause delimiters.
 * Other keywords (like :string, :number) are treated as values.
 * Returns Map<string, any[]>.
 */
function parseKeywordClauses(args) {
	const clauses = new Map();
	let currentKey = null;
	let currentValues = [];

	for (const arg of args) {
		if (isKeyword(arg) && FUNC_CLAUSE_KEYS.has(arg.value)) {
			if (currentKey !== null) {
				clauses.set(currentKey, currentValues);
			}
			currentKey = arg.value;
			currentValues = [];
		} else {
			currentValues.push(arg);
		}
	}
	if (currentKey !== null) {
		clauses.set(currentKey, currentValues);
	}
	return clauses;
}

/**
 * Parse a destructuring pattern list: (object :type name ...) or (array :type name ...)
 * Returns { destructured: true, kind, fields, rest? }
 */
function parseDestructuredParam(listNode) {
	const values = listNode.values;
	if (values.length === 0) {
		throw new Error(
			"empty destructuring pattern — at least one field required",
		);
	}
	const head = values[0];
	if (head.type !== "atom" || (head.value !== "object" && head.value !== "array")) {
		throw new Error(
			`expected 'object' or 'array' at head of destructuring pattern, got '${head.value ?? head.type}'`,
		);
	}
	const kind = head.value;
	const inner = values.slice(1);

	if (inner.length === 0) {
		throw new Error(
			"empty destructuring pattern — at least one field required",
		);
	}

	if (kind === "object") {
		return parseObjectDestructure(inner, listNode);
	}
	return parseArrayDestructure(inner, listNode);
}

function parseObjectDestructure(values, _parentNode) {
	const fields = [];
	let i = 0;
	while (i < values.length) {
		const cur = values[i];
		// Check for deferred features in type position
		if (isArray(cur)) {
			const headVal = cur.values[0];
			const headName = headVal?.type === "atom" ? headVal.value : "";
			if (headName === "default") {
				if (cur.values.length !== 4) {
					throw new Error(
						"default in destructured params requires 3 arguments: (default :type name value)",
					);
				}
				const defTypeKw = cur.values[1];
				const defName = cur.values[2];
				const defValue = cur.values[3];
				if (!isKeyword(defTypeKw)) {
					throw new Error("default: first argument must be a type keyword");
				}
				if (defName.type !== "atom") {
					throw new Error("default: second argument must be a parameter name");
				}
				fields.push({ typeKw: defTypeKw, name: defName, defaultValue: defValue });
				i += 1;
				continue;
			}
			if (headName === "alias") {
				// (alias :type alias-name (object/array ...))
				if (cur.values.length < 4) {
					throw new Error(
						"alias in destructured params requires: (alias :type name (object/array ...))",
					);
				}
				const aliasTypeKw = cur.values[1];
				const aliasName = cur.values[2];
				const innerPattern = cur.values[3];
				if (!isKeyword(aliasTypeKw)) {
					throw new Error("alias: first argument must be a type keyword");
				}
				if (aliasName.type !== "atom") {
					throw new Error("alias: second argument must be a name");
				}
				if (!isArray(innerPattern)) {
					throw new Error(
						"alias: third argument must be a destructuring pattern (object/array ...)",
					);
				}
				const nestedParam = parseDestructuredParam(innerPattern);
				fields.push({
					nested: true,
					alias: aliasName,
					typeKw: aliasTypeKw,
					pattern: nestedParam,
				});
				i += 1;
				continue;
			}
			if (headName === "object" || headName === "array") {
				throw new Error(
					"nested pattern in object destructuring must use alias to specify the property name: (alias :type name (object/array ...))",
				);
			}
			throw new Error(
				`expected type keyword at position ${i} in destructuring pattern`,
			);
		}
		if (!isKeyword(cur)) {
			if (cur.type === "atom") {
				throw new Error(
					`field '${cur.value}' missing type annotation (use :any to opt out)`,
				);
			}
			throw new Error(
				`expected type keyword at position ${i} in destructuring pattern`,
			);
		}
		if (i + 1 >= values.length) {
			throw new Error(
				`type keyword :${cur.value} has no field name in destructuring pattern`,
			);
		}
		const nameNode = values[i + 1];
		if (isArray(nameNode)) {
			throw new Error("field name must be an atom");
		}
		if (nameNode.type !== "atom") {
			throw new Error("field name must be an atom");
		}
		fields.push({ typeKw: cur, name: nameNode });
		i += 2;
	}
	return { destructured: true, kind: "object", fields };
}

function parseArrayDestructure(values, _parentNode) {
	const fields = [];
	let rest = null;
	let i = 0;
	while (i < values.length) {
		const cur = values[i];
		// Skip element: _
		if (cur.type === "atom" && cur.value === "_") {
			fields.push({ skip: true, name: cur });
			i += 1;
			continue;
		}
		// Rest element: (rest :type name)
		if (isArray(cur)) {
			const headVal = cur.values[0];
			const headName = headVal?.type === "atom" ? headVal.value : "";
			if (headName === "rest") {
				if (cur.values.length !== 3) {
					throw new Error("rest element must be (rest :type name)");
				}
				if (i + 1 !== values.length) {
					throw new Error(
						"rest element must be last in array destructuring",
					);
				}
				if (!isKeyword(cur.values[1])) {
					throw new Error("rest element must be (rest :type name)");
				}
				if (cur.values[2].type !== "atom") {
					throw new Error("rest element must be (rest :type name)");
				}
				rest = { typeKw: cur.values[1], name: cur.values[2] };
				i += 1;
				continue;
			}
			if (headName === "default") {
				if (cur.values.length !== 4) {
					throw new Error(
						"default in destructured params requires 3 arguments: (default :type name value)",
					);
				}
				const defTypeKw = cur.values[1];
				const defName = cur.values[2];
				const defValue = cur.values[3];
				if (!isKeyword(defTypeKw)) {
					throw new Error("default: first argument must be a type keyword");
				}
				if (defName.type !== "atom") {
					throw new Error("default: second argument must be a parameter name");
				}
				fields.push({ typeKw: defTypeKw, name: defName, defaultValue: defValue });
				i += 1;
				continue;
			}
			if (headName === "object" || headName === "array") {
				// Positional nested pattern — no alias needed in array context
				const nestedParam = parseDestructuredParam(cur);
				fields.push({
					nested: true,
					alias: null,
					typeKw: null,
					pattern: nestedParam,
				});
				i += 1;
				continue;
			}
			if (headName === "alias") {
				if (cur.values.length < 4) {
					throw new Error(
						"alias in destructured params requires: (alias :type name (object/array ...))",
					);
				}
				const aliasTypeKw = cur.values[1];
				const aliasName = cur.values[2];
				const innerPattern = cur.values[3];
				if (!isKeyword(aliasTypeKw)) {
					throw new Error("alias: first argument must be a type keyword");
				}
				if (aliasName.type !== "atom") {
					throw new Error("alias: second argument must be a name");
				}
				if (!isArray(innerPattern)) {
					throw new Error(
						"alias: third argument must be a destructuring pattern (object/array ...)",
					);
				}
				const nestedParam = parseDestructuredParam(innerPattern);
				fields.push({
					nested: true,
					alias: aliasName,
					typeKw: aliasTypeKw,
					pattern: nestedParam,
				});
				i += 1;
				continue;
			}
			throw new Error(
				`unexpected list in array destructuring at position ${i}`,
			);
		}
		// Typed element: :type name
		if (isKeyword(cur)) {
			if (i + 1 >= values.length) {
				throw new Error(
					`type keyword :${cur.value} has no element name`,
				);
			}
			const nameNode = values[i + 1];
			if (nameNode.type !== "atom") {
				throw new Error("element name must be an atom");
			}
			fields.push({ typeKw: cur, name: nameNode });
			i += 2;
			continue;
		}
		// Bare name without type keyword
		if (cur.type === "atom") {
			throw new Error(
				`field '${cur.value}' missing type annotation (use :any to opt out)`,
			);
		}
		throw new Error(
			`expected type keyword, _, or (rest ...) at position ${i} in array destructuring`,
		);
	}
	return { destructured: true, kind: "array", fields, rest };
}

// --- Param shape helpers ---

/** Get kernel param name nodes for a function signature. */
function paramNameNodes(p) {
	if (p.destructured) {
		if (p.kind === "object") {
			const elems = p.fields.map((f) => {
				if (f.nested) {
					const innerKernel = paramNameNodes(f.pattern)[0];
					if (f.alias) {
						return array(sym("alias"), f.alias, innerKernel);
					}
					return innerKernel;
				}
				if (f.defaultValue) {
					return array(sym("default"), f.name, f.defaultValue);
				}
				return f.name;
			});
			return [array(sym("object"), ...elems)];
		}
		if (p.kind === "array") {
			const elems = p.fields.map((f) => {
				if (f.skip) return sym("_");
				if (f.nested) {
					const innerKernel = paramNameNodes(f.pattern)[0];
					if (f.alias) {
						return array(sym("alias"), f.alias, innerKernel);
					}
					return innerKernel;
				}
				if (f.defaultValue) return array(sym("default"), f.name, f.defaultValue);
				return f.name;
			});
			if (p.rest) {
				elems.push(array(sym("rest"), p.rest.name));
			}
			return [array(sym("array"), ...elems)];
		}
	}
	// Simple param — handle default and rest
	if (p.defaultValue) {
		return [array(sym("default"), p.name, p.defaultValue)];
	}
	if (p.isRest) {
		return [array(sym("rest"), p.name)];
	}
	return [p.name];
}

/** Get type check assertions for a param. */
function paramTypeChecks(p, funcName) {
	if (p.destructured) {
		const checks = [];
		const allFields = [
			...p.fields.filter((f) => !f.skip),
			...(p.rest ? [p.rest] : []),
		];
		for (const f of allFields) {
			if (f.nested) {
				// Recurse into nested pattern for leaf-level checks
				checks.push(...paramTypeChecks(f.pattern, funcName));
				// Type-check the alias binding itself if not :any
				if (f.typeKw && f.typeKw.value !== "any") {
					const check = buildTypeCheck(f.alias, f.typeKw, funcName, "arg");
					if (check) checks.push(check);
				}
				continue;
			}
			const check = buildTypeCheck(f.name, f.typeKw, funcName, "arg");
			if (check) checks.push(check);
		}
		return checks;
	}
	// Rest param: per-element type check via for-of loop
	if (p.isRest) {
		if (p.typeKw.value === "any") return [];
		const elVar = gensym("el");
		const innerCheck = buildTypeCheck(elVar, p.typeKw, funcName, "rest arg");
		if (!innerCheck) return [];
		return [array(sym("for-of"), elVar, p.name, innerCheck)];
	}
	const check = buildTypeCheck(p.name, p.typeKw, funcName, "arg");
	return check ? [check] : [];
}

/** Get the dispatch type string for multi-clause dispatch. */
function paramDispatchType(p) {
	if (p.destructured) return p.kind;
	return p.typeKw.value;
}

/** Get all bound name nodes for a param. */
function paramBoundNames(p) {
	if (p.destructured) {
		const names = [];
		for (const f of p.fields) {
			if (f.skip) continue;
			if (f.nested) {
				if (f.alias) names.push(f.alias);
				names.push(...paramBoundNames(f.pattern));
				continue;
			}
			names.push(f.name);
		}
		if (p.rest) names.push(p.rest.name);
		return names;
	}
	return [p.name];
}

/**
 * Parse a top-level (default :type name value) in :args.
 */
function parseDefaultParam(listNode) {
	const vals = listNode.values;
	if (vals.length !== 4) {
		throw new Error(
			"(default) in :args requires exactly 3 elements: :type name value",
		);
	}
	if (!isKeyword(vals[1])) {
		throw new Error("(default) first element must be a type keyword");
	}
	if (vals[2].type !== "atom") {
		throw new Error("(default) second element must be a name");
	}
	return { typeKw: vals[1], name: vals[2], defaultValue: vals[3] };
}

/**
 * Parse a top-level (rest :type name) in :args.
 */
function parseRestParam(listNode) {
	const vals = listNode.values;
	if (vals.length !== 3) {
		throw new Error(
			"(rest) in :args requires exactly 2 elements: :type name",
		);
	}
	if (!isKeyword(vals[1])) {
		throw new Error("(rest) first element must be a type keyword");
	}
	if (vals[2].type !== "atom") {
		throw new Error("(rest) second element must be a name");
	}
	return { typeKw: vals[1], name: vals[2], isRest: true };
}

/**
 * Parse typed parameter list: (:type name :type name ...) → [{typeKw, name}, ...]
 * Also accepts destructuring patterns, (default ...), and (rest ...).
 */
function parseTypedParams(paramList) {
	const params = [];
	const values = paramList.values;
	let i = 0;
	while (i < values.length) {
		if (isArray(values[i])) {
			const headVal = values[i].values[0];
			const headName = headVal?.type === "atom" ? headVal.value : "";
			if (headName === "object" || headName === "array") {
				params.push(parseDestructuredParam(values[i]));
			} else if (headName === "default") {
				params.push(parseDefaultParam(values[i]));
			} else if (headName === "rest") {
				params.push(parseRestParam(values[i]));
			} else {
				throw new Error(
					`expected 'object', 'array', 'default', or 'rest' at head of sub-form in :args, got '${headName || values[i].type}'`,
				);
			}
			i += 1;
		} else if (isKeyword(values[i])) {
			// Simple param — :type name pair
			if (i + 1 >= values.length) {
				throw new Error(
					`type keyword :${values[i].value} has no parameter name`,
				);
			}
			params.push({ typeKw: values[i], name: values[i + 1] });
			i += 2;
		} else {
			throw new Error(
				`expected type keyword or sub-form at position ${i}, got ${values[i]?.type ?? "nothing"}`,
			);
		}
	}
	// Validate rest constraints
	const restParams = params.filter((p) => p.isRest);
	if (restParams.length > 1) {
		throw new Error("only one rest parameter allowed");
	}
	if (restParams.length === 1) {
		const lastParam = params[params.length - 1];
		if (!lastParam.isRest) {
			throw new Error("rest parameter must be the last parameter");
		}
	}
	return params;
}

/**
 * Replace all occurrences of ~ (tilde atom) in an AST with a replacement node.
 */
function replaceTilde(node, replacement) {
	if (!node) return node;
	if (node.type === "atom" && node.value === "~") return replacement;
	if (node.type === "list") {
		return {
			type: "list",
			values: node.values.map((v) => replaceTilde(v, replacement)),
		};
	}
	return node;
}

/**
 * Compile a match pattern against a target symbol.
 * Returns { checks: AST[], bindings: AST[] }.
 * checks are conditions (to be &&'d together).
 * bindings are (const ...) forms.
 */
function compilePattern(pattern, targetSym) {
	// Wildcard
	if (pattern.type === "atom" && pattern.value === "_") {
		return { checks: [], bindings: [] };
	}

	// Literal: number
	if (pattern.type === "number") {
		return {
			checks: [array(sym("==="), targetSym, pattern)],
			bindings: [],
		};
	}

	// Literal: string
	if (pattern.type === "string") {
		return {
			checks: [array(sym("==="), targetSym, pattern)],
			bindings: [],
		};
	}

	// Literal: keyword → string comparison
	if (pattern.type === "keyword") {
		return {
			checks: [
				array(sym("==="), targetSym, { type: "string", value: pattern.value }),
			],
			bindings: [],
		};
	}

	// Literal atoms: true, false, null, undefined
	if (
		pattern.type === "atom" &&
		(pattern.value === "true" ||
			pattern.value === "false" ||
			pattern.value === "null" ||
			pattern.value === "undefined")
	) {
		return {
			checks: [array(sym("==="), targetSym, pattern)],
			bindings: [],
		};
	}

	// Zero-field ADT constructor (PascalCase bare atom)
	if (pattern.type === "atom" && isPascalCase(pattern.value)) {
		return {
			checks: [
				array(sym("==="), sym(`${targetSym.value}:tag`), {
					type: "string",
					value: pattern.value,
				}),
			],
			bindings: [],
		};
	}

	// Simple binding (lowercase bare atom, not wildcard)
	if (pattern.type === "atom") {
		return {
			checks: [],
			bindings: [array(sym("const"), pattern, targetSym)],
		};
	}

	// List patterns
	if (pattern.type === "list" && pattern.values.length > 0) {
		const head = pattern.values[0];

		// Structural obj pattern: (obj :key binding :key binding ...)
		if (head.type === "atom" && head.value === "obj") {
			const checks = [
				array(sym("==="), array(sym("typeof"), targetSym), {
					type: "string",
					value: "object",
				}),
				array(sym("!=="), targetSym, sym("null")),
			];
			const bindings = [];
			const pairs = pattern.values.slice(1);
			for (let i = 0; i < pairs.length; i += 2) {
				const key = pairs[i];
				const binding = pairs[i + 1];
				if (!isKeyword(key)) {
					throw new Error(
						`match obj pattern: expected keyword, got ${key?.type}`,
					);
				}
				const keyStr = { type: "string", value: key.value };
				checks.push(array(sym("in"), keyStr, targetSym));
				// If binding is a literal, add equality check instead of binding
				if (
					binding.type === "number" ||
					binding.type === "string" ||
					(binding.type === "atom" &&
						(binding.value === "true" ||
							binding.value === "false" ||
							binding.value === "null"))
				) {
					checks.push(
						array(sym("==="), sym(`${targetSym.value}:${key.value}`), binding),
					);
				} else if (binding.type === "atom" && binding.value !== "_") {
					bindings.push(
						array(
							sym("const"),
							binding,
							sym(`${targetSym.value}:${key.value}`),
						),
					);
				}
				// _ in obj pattern — just check key exists, no binding
			}
			return { checks, bindings };
		}

		// ADT constructor pattern: (ConstructorName binding1 binding2 ...)
		if (head.type === "atom" && isPascalCase(head.value)) {
			const ctorName = head.value;
			const fieldNames = typeRegistry.get(ctorName);
			const patternBindings = pattern.values.slice(1);

			const checks = [
				array(sym("==="), sym(`${targetSym.value}:tag`), {
					type: "string",
					value: ctorName,
				}),
			];
			const bindings = [];

			if (fieldNames) {
				// Type registry available — use named field access
				for (let i = 0; i < patternBindings.length; i++) {
					const fieldName = fieldNames[i];
					if (!fieldName) break;
					const binding = patternBindings[i];
					const fieldAccess = sym(`${targetSym.value}:${fieldName}`);

					if (binding.type === "atom" && binding.value === "_") {
						// Wildcard — no binding
					} else if (
						binding.type === "list" &&
						binding.values.length > 0 &&
						isPascalCase(binding.values[0].value)
					) {
						// Nested ADT pattern
						const nestedTarget = gensym("t");
						bindings.push(array(sym("const"), nestedTarget, fieldAccess));
						const nested = compilePattern(binding, nestedTarget);
						checks.push(...nested.checks);
						bindings.push(...nested.bindings);
					} else if (binding.type === "atom") {
						bindings.push(array(sym("const"), binding, fieldAccess));
					} else {
						// Literal in pattern position — equality check
						checks.push(array(sym("==="), fieldAccess, binding));
					}
				}
			} else {
				// No type registry — positional field access via Object.values
				// This is a fallback; with type registry it shouldn't happen for well-typed code
				for (let i = 0; i < patternBindings.length; i++) {
					const binding = patternBindings[i];
					const fieldAccess = array(
						sym("get"),
						array(sym("Object:values"), targetSym),
						{ type: "number", value: i + 1 },
					); // +1 to skip tag
					if (binding.type === "atom" && binding.value !== "_") {
						bindings.push(array(sym("const"), binding, fieldAccess));
					}
				}
			}

			return { checks, bindings };
		}
	}

	throw new Error(`match: unrecognized pattern: ${formatSExpr(pattern)}`);
}

/**
 * Build an && chain from an array of check AST nodes.
 */
function andChain(checks) {
	if (checks.length === 0) return null;
	if (checks.length === 1) return checks[0];
	let result = checks[0];
	for (let i = 1; i < checks.length; i++) {
		result = array(sym("&&"), result, checks[i]);
	}
	return result;
}

/**
 * Register all surface form macros into the macro environment.
 * @param {Map<string, Function>} macroEnv
 */
export function registerSurfaceMacros(macroEnv) {
	// --- Shared helpers (scoped to registerSurfaceMacros for access to sym, array, etc.) ---

	/**
	 * Wrap body forms so the last expression is returned.
	 * Empty → [], single → [(return expr)], multiple → [...init, (return last)].
	 */
	function wrapReturnLast(bodyForms) {
		if (bodyForms.length === 0) return [];
		if (bodyForms.length === 1) return [array(sym("return"), bodyForms[0])];
		return [...bodyForms.slice(0, -1), array(sym("return"), bodyForms[bodyForms.length - 1])];
	}

	/**
	 * Build a threading expression (thread-first or thread-last).
	 * position: 'first' — insert threaded as first arg after fn
	 * position: 'last'  — insert threaded as last arg
	 */
	function buildThread(args, position) {
		if (args.length < 2) {
			const name = position === "first" ? "->" : "->>";
			throw new Error(`${name} requires at least 2 arguments: (${name} value step...)`);
		}
		let threaded = args[0];
		for (let i = 1; i < args.length; i++) {
			const step = args[i];
			if (isKeyword(step)) {
				threaded = array(sym("."), threaded, sym(step.value));
			} else if (isArray(step) && step.values.length > 0 && isKeyword(step.values[0])) {
				const [kw, ...rest] = step.values;
				threaded = array(sym("."), threaded, sym(kw.value), ...rest);
			} else if (isArray(step)) {
				if (position === "first") {
					const [fn, ...rest] = step.values;
					threaded = array(fn, threaded, ...rest);
				} else {
					threaded = array(...step.values, threaded);
				}
			} else {
				threaded = array(step, threaded);
			}
		}
		return threaded;
	}

	/**
	 * Build a nil-safe threading expression (some-> or some->>).
	 * position: 'first' or 'last' — same insertion semantics as buildThread.
	 */
	function buildSomeThread(args, position) {
		const name = position === "first" ? "some->" : "some->>";
		if (args.length < 2) {
			throw new Error(`${name} requires at least 2 arguments`);
		}
		const stmts = [];
		let prevVar = gensym("t");
		stmts.push(array(sym("const"), prevVar, args[0]));
		stmts.push(
			array(
				sym("if"),
				array(sym("=="), prevVar, sym("null")),
				array(sym("return"), prevVar),
			),
		);

		for (let i = 1; i < args.length; i++) {
			const step = args[i];
			let callExpr;
			if (isKeyword(step)) {
				callExpr = array(sym("."), prevVar, sym(step.value));
			} else if (isArray(step) && step.values.length > 0 && isKeyword(step.values[0])) {
				const [kw, ...rest] = step.values;
				callExpr = array(sym("."), prevVar, sym(kw.value), ...rest);
			} else if (isArray(step)) {
				if (position === "first") {
					const [fn, ...rest] = step.values;
					callExpr = array(fn, prevVar, ...rest);
				} else {
					callExpr = array(...step.values, prevVar);
				}
			} else {
				callExpr = array(step, prevVar);
			}

			if (i === args.length - 1) {
				stmts.push(array(sym("return"), callExpr));
			} else {
				const nextVar = gensym("t");
				stmts.push(array(sym("const"), nextVar, callExpr));
				stmts.push(
					array(
						sym("if"),
						array(sym("=="), nextVar, sym("null")),
						array(sym("return"), nextVar),
					),
				);
				prevVar = nextVar;
			}
		}

		const arrowFn = array(sym("=>"), array(), ...stmts);
		return array(arrowFn);
	}

	/**
	 * Compile a let-binding pattern (for if-let / when-let).
	 * Detects ADT constructor patterns (PascalCase), obj patterns, and simple bindings.
	 * Returns { condition, bindings } where condition is the test AST node
	 * and bindings is an array of (const ...) forms.
	 */
	function compileLetPattern(pattern, tempVar) {
		if (
			isArray(pattern) &&
			pattern.values.length > 0 &&
			pattern.values[0].type === "atom" &&
			isPascalCase(pattern.values[0].value)
		) {
			// ADT constructor pattern
			const { checks, bindings } = compilePattern(pattern, tempVar);
			return { condition: andChain(checks), bindings };
		}
		if (
			isArray(pattern) &&
			pattern.values.length > 0 &&
			pattern.values[0].type === "atom" &&
			pattern.values[0].value === "obj"
		) {
			// Structural obj pattern
			const { checks, bindings } = compilePattern(pattern, tempVar);
			return { condition: andChain(checks), bindings };
		}
		if (pattern.type === "atom" && !isPascalCase(pattern.value)) {
			// Simple binding — nil check (loose != to catch null and undefined)
			const condition = kernelArray(sym("!="), tempVar, sym("null"));
			const bindings = [array(sym("const"), pattern, tempVar)];
			return { condition, bindings };
		}
		return null; // unrecognized
	}

	// --- bind ---
	// (bind name value) → (const name value)
	// (bind :type name value) → (const name value) + type check
	macroEnv.set("bind", (...args) => {
		if (args.length < 2) {
			throw new Error("bind requires at least 2 arguments: (bind name value)");
		}
		if (isKeyword(args[0])) {
			if (args.length < 3) {
				throw new Error(
					"bind with type annotation requires 3 arguments: (bind :type name value)",
				);
			}
			const typeKw = args[0];
			const nameNode = args[1];
			const valueNode = args[2];
			const typeName = typeKw.value;
			const constDecl = array(sym("const"), nameNode, valueNode);

			// :any — no check
			if (typeName === "any") {
				return constDecl;
			}

			// Static check: literal initializer
			const literalType = getLiteralType(valueNode);
			if (literalType !== null) {
				if (!typeMatchesLiteral(typeName, literalType)) {
					throw new Error(
						`bind '${nameNode.value}': type annotation is :${typeName} but initializer is a ${literalType} literal. Remove the annotation or fix the type.`,
					);
				}
				// Type-compatible literal — no runtime check needed
				return constDecl;
			}

			// Non-literal — emit runtime check
			const check = buildTypeCheck(nameNode, typeKw, "bind", "");
			if (check === null) {
				return constDecl;
			}
			return array(sym("block"), constDecl, check);
		}
		return array(sym("const"), args[0], args[1]);
	});

	// --- obj ---
	// (obj :name "Duncan" :age 42) → (object (name "Duncan") (age 42))
	macroEnv.set("obj", (...args) => {
		const pairs = [];
		for (let i = 0; i < args.length; i += 2) {
			if (!isKeyword(args[i])) {
				throw new Error(
					`obj: expected keyword at position ${i}, got ${args[i]?.type ?? "nothing"}`,
				);
			}
			if (i + 1 >= args.length) {
				throw new Error(`obj: keyword :${args[i].value} has no value`);
			}
			pairs.push(kernelArray(sym(args[i].value), args[i + 1]));
		}
		return array(sym("object"), ...pairs);
	});

	// --- cell ---
	// (cell value) → (object (value value))
	macroEnv.set("cell", (...args) => {
		if (args.length !== 1) {
			throw new Error("cell requires exactly 1 argument: (cell value)");
		}
		return array(sym("object"), array(sym("value"), args[0]));
	});

	// --- express ---
	// (express c) → c:value
	macroEnv.set("express", (...args) => {
		if (args.length !== 1) {
			throw new Error("express requires exactly 1 argument: (express cell)");
		}
		const cell = args[0];
		if (cell.type !== "atom") {
			throw new Error("express: argument must be a symbol");
		}
		return sym(`${cell.value}:value`);
	});

	// --- swap! ---
	// (swap! c f) → (= c:value (f c:value))
	// (swap! c f a b) → (= c:value (f c:value a b))
	macroEnv.set("swap!", (...args) => {
		if (args.length < 2) {
			throw new Error("swap! requires at least 2 arguments: (swap! cell fn)");
		}
		const cell = args[0];
		if (cell.type !== "atom") {
			throw new Error("swap!: first argument must be a symbol");
		}
		const fn = args[1];
		const extraArgs = args.slice(2);
		const cellValue = sym(`${cell.value}:value`);
		return kernelArray(sym("="), cellValue, array(fn, cellValue, ...extraArgs));
	});

	// --- reset! ---
	// (reset! c v) → (= c:value v)
	macroEnv.set("reset!", (...args) => {
		if (args.length !== 2) {
			throw new Error(
				"reset! requires exactly 2 arguments: (reset! cell value)",
			);
		}
		const cell = args[0];
		if (cell.type !== "atom") {
			throw new Error("reset!: first argument must be a symbol");
		}
		return kernelArray(sym("="), sym(`${cell.value}:value`), args[1]);
	});

	// --- set! ---
	// (set! target:prop value) → (= target:prop value) [kernel assignment]
	// Target must be colon-syntax (member expression), not a bare binding.
	macroEnv.set("set!", (...args) => {
		if (args.length !== 2) {
			throw new Error(
				"set! requires exactly 2 arguments: (set! target:prop value)",
			);
		}
		const target = args[0];
		if (target.type !== "atom" || !target.value.includes(":")) {
			throw new Error(
				"set! requires a property path (e.g., obj:prop), not a bare binding. " +
					"Use (bind x val) for new bindings, (reset! cell val) for cells.",
			);
		}
		return kernelArray(sym("="), target, args[1]);
	});

	// --- set-symbol! ---
	// (set-symbol! obj key value) → (= (get obj key) value) [kernel assignment]
	// For Symbol-keyed property mutation. Reading uses (get obj key) directly.
	// TODO: deprecate when surface/kernel syntaxes are separated; remove the release after that.
	macroEnv.set("set-symbol!", (...args) => {
		if (args.length !== 3) {
			throw new Error(
				"set-symbol! requires exactly 3 arguments: (set-symbol! obj key value)",
			);
		}
		const target = array(sym("get"), args[0], args[1]);
		return kernelArray(sym("="), target, args[2]);
	});

	// --- = (strict equality) ---
	// (= a b) → (=== a b)
	// (= a b c) → (&& (=== a b) (=== b c))
	macroEnv.set("=", (...args) => {
		if (args.length < 2) {
			throw new Error("= requires at least 2 arguments: (= a b)");
		}
		if (args.length === 2) {
			return array(sym("==="), args[0], args[1]);
		}
		// Variadic: (= a b c) → (&&(=== a b) (=== b c))
		const checks = [];
		for (let i = 0; i < args.length - 1; i++) {
			checks.push(array(sym("==="), args[i], args[i + 1]));
		}
		let result = checks[0];
		for (let i = 1; i < checks.length; i++) {
			result = array(sym("&&"), result, checks[i]);
		}
		return result;
	});

	// --- != (strict inequality) ---
	// (!= a b) → (!== a b)
	macroEnv.set("!=", (...args) => {
		if (args.length !== 2) {
			throw new Error("!= requires exactly 2 arguments: (!= a b)");
		}
		return array(sym("!=="), args[0], args[1]);
	});

	// --- and (logical AND) ---
	// (and a b) → (&& a b)
	// (and a b c d) → (&& (&& (&& a b) c) d)
	macroEnv.set("and", (...args) => {
		if (args.length < 2) {
			throw new Error("and requires at least 2 arguments: (and a b)");
		}
		let result = args[0];
		for (let i = 1; i < args.length; i++) {
			result = array(sym("&&"), result, args[i]);
		}
		return result;
	});

	// --- or (logical OR) ---
	// (or a b) → (|| a b)
	// (or a b c d) → (|| (|| (|| a b) c) d)
	macroEnv.set("or", (...args) => {
		if (args.length < 2) {
			throw new Error("or requires at least 2 arguments: (or a b)");
		}
		let result = args[0];
		for (let i = 1; i < args.length; i++) {
			result = array(sym("||"), result, args[i]);
		}
		return result;
	});

	// --- not (logical NOT) ---
	// (not x) → (! x)
	macroEnv.set("not", (...args) => {
		if (args.length !== 1) {
			throw new Error("not requires exactly 1 argument: (not x)");
		}
		return array(sym("!"), args[0]);
	});

	// --- -> (thread-first) ---
	macroEnv.set("->", (...args) => buildThread(args, "first"));

	// --- ->> (thread-last) ---
	macroEnv.set("->>", (...args) => buildThread(args, "last"));

	// --- assoc ---
	macroEnv.set("assoc", (...args) => {
		if (args.length < 3) {
			throw new Error(
				"assoc requires at least 3 arguments: (assoc obj :key value)",
			);
		}
		const obj = args[0];
		const pairs = [];
		for (let i = 1; i < args.length; i += 2) {
			if (!isKeyword(args[i])) {
				throw new Error(
					`assoc: expected keyword at position ${i}, got ${args[i]?.type ?? "nothing"}`,
				);
			}
			if (i + 1 >= args.length) {
				throw new Error(`assoc: keyword :${args[i].value} has no value`);
			}
			pairs.push(array(sym(args[i].value), args[i + 1]));
		}
		return array(sym("object"), array(sym("spread"), obj), ...pairs);
	});

	// --- dissoc ---
	macroEnv.set("dissoc", (...args) => {
		if (args.length < 2) {
			throw new Error(
				"dissoc requires at least 2 arguments: (dissoc obj :key)",
			);
		}
		const obj = args[0];
		const aliasPatterns = [];
		for (let i = 1; i < args.length; i++) {
			if (!isKeyword(args[i])) {
				throw new Error(
					`dissoc: expected keyword at position ${i}, got ${args[i]?.type ?? "nothing"}`,
				);
			}
			const discardVar = gensym("_");
			aliasPatterns.push(array(sym("alias"), sym(args[i].value), discardVar));
		}
		const restVar = gensym("rest");
		const pattern = array(
			sym("object"),
			...aliasPatterns,
			array(sym("rest"), restVar),
		);
		const binding = array(sym("const"), pattern, obj);
		const arrowBody = array(sym("=>"), array(), binding, array(sym("return"), restVar));
		return array(arrowBody);
	});

	// --- conj ---
	macroEnv.set("conj", (...args) => {
		if (args.length !== 2) {
			throw new Error("conj requires exactly 2 arguments: (conj array value)");
		}
		return array(sym("array"), array(sym("spread"), args[0]), args[1]);
	});

	// ===================================================================
	// js: namespace interop (DD-15)
	// ===================================================================

	// --- js:call ---
	// (js:call obj:method args...) → (obj:method args...)
	macroEnv.set("js:call", (...args) => {
		if (args.length < 1) {
			throw new Error("js:call requires at least a method reference");
		}
		return array(args[0], ...args.slice(1));
	});

	// --- js:bind ---
	// (js:bind obj:method obj) → (obj:method:bind obj)
	macroEnv.set("js:bind", (...args) => {
		if (args.length !== 2) {
			throw new Error(
				"js:bind requires exactly 2 arguments: (js:bind obj:method obj)",
			);
		}
		if (args[0].type !== "atom") {
			throw new Error("js:bind: first argument must be a method reference");
		}
		return array(sym(`${args[0].value}:bind`), args[1]);
	});

	// --- js:eval ---
	// (js:eval code) → (eval code)
	macroEnv.set("js:eval", (...args) => {
		if (args.length !== 1) {
			throw new Error("js:eval requires exactly 1 argument");
		}
		return array(sym("eval"), args[0]);
	});

	// --- js:eq ---
	// (js:eq a b) → (== a b)
	macroEnv.set("js:eq", (...args) => {
		if (args.length !== 2) {
			throw new Error("js:eq requires exactly 2 arguments");
		}
		return array(sym("=="), args[0], args[1]);
	});

	// --- js:typeof ---
	// (js:typeof x) → (typeof x)
	macroEnv.set("js:typeof", (...args) => {
		if (args.length !== 1) {
			throw new Error("js:typeof requires exactly 1 argument");
		}
		return array(sym("typeof"), args[0]);
	});

	// ===================================================================
	// Phase 2: Complex Surface Forms
	// ===================================================================

	// --- fn / lambda ---
	// (fn (:number x :number y) (+ x y)) → (=> (x y) <type-checks> (+ x y))
	// (fn () (Date:now)) → (=> () (Date:now))
	const fnMacro = (...args) => {
		if (args.length < 2) {
			throw new Error(
				"fn requires at least 2 arguments: (fn (params) body...)",
			);
		}
		const paramList = args[0];
		if (!isArray(paramList)) {
			throw new Error("fn: first argument must be a parameter list");
		}
		const bodyForms = args.slice(1);

		// Parse typed params
		const params = parseTypedParams(paramList);
		const pNames = params.flatMap((p) => paramNameNodes(p));

		// Build type checks
		const typeChecks = [];
		for (const p of params) {
			typeChecks.push(...paramTypeChecks(p, "anonymous"));
		}

		// When type checks are present, the arrow gets a block body, so we must
		// wrap the last body expression in (return ...) to preserve the return value.
		if (typeChecks.length > 0) {
			return array(sym("=>"), array(...pNames), ...typeChecks, ...wrapReturnLast(bodyForms));
		}
		return array(sym("=>"), array(...pNames), ...typeChecks, ...bodyForms);
	};

	macroEnv.set("fn", fnMacro);
	macroEnv.set("lambda", fnMacro);

	// --- genfunc / genfn (generator functions) ---

	/**
	 * Recursively walk an AST node and instrument (yield expr) forms
	 * with a type check on the yielded value. Leaves (yield*) unchanged.
	 */
	function instrumentYields(node, yieldsType, funcName) {
		if (!node || node.type !== "list") return node;
		const vals = node.values;
		if (vals.length === 0) return node;

		// (yield expr) → wrap in IIFE: (yield ((=> () (const __v expr) check (return __v))))
		if (vals[0].type === "atom" && vals[0].value === "yield" && vals.length >= 2) {
			const yieldedExpr = instrumentYields(vals[1], yieldsType, funcName);
			const vVar = gensym("yv");
			const check = buildTypeCheck(vVar, yieldsType, funcName, "yield");
			if (check) {
				// IIFE that checks and returns the value
				const iife = array(
					array(sym("=>"), array(),
						array(sym("const"), vVar, yieldedExpr),
						check,
						array(sym("return"), vVar),
					),
				);
				return array(sym("yield"), iife);
			}
			return array(sym("yield"), yieldedExpr);
		}

		// (yield* ...) — leave as-is, delegate responsibility
		if (vals[0].type === "atom" && vals[0].value === "yield*") {
			return node;
		}

		// Recurse into all sub-expressions
		return {
			...node,
			values: vals.map((v) => instrumentYields(v, yieldsType, funcName)),
		};
	}

	// (genfunc name :args (...) :yields :type :body ...)
	macroEnv.set("genfunc", (...args) => {
		if (args.length < 2) {
			throw new Error("genfunc requires at least a name and :yields/:body");
		}
		const funcNameNode = args[0];
		if (funcNameNode.type !== "atom") {
			throw new Error("genfunc: first argument must be a function name");
		}
		const funcName = funcNameNode.value;
		const clauseArgs = args.slice(1);
		const clauses = parseKeywordClauses(clauseArgs);
		const argsClause = clauses.get("args");
		const yieldsClause = clauses.get("yields");
		const _returnsClause = clauses.get("returns");
		const preClause = clauses.get("pre");
		const _postClause = clauses.get("post");
		const bodyClause = clauses.get("body");

		if (!bodyClause || bodyClause.length === 0) {
			throw new Error(`genfunc ${funcName}: :body is required`);
		}

		// Parse params
		let params = [];
		if (argsClause && argsClause.length === 1 && isArray(argsClause[0])) {
			params = parseTypedParams(argsClause[0]);
		}
		const pNames = params.flatMap((p) => paramNameNodes(p));

		// Build generator body
		const bodyStmts = [];

		// Type checks for params
		for (const p of params) {
			bodyStmts.push(...paramTypeChecks(p, funcName));
		}

		// Pre-condition
		if (preClause && preClause.length > 0) {
			const preExpr = preClause[0];
			const preMsg = `${funcName}: pre-condition failed: ${formatSExpr(preExpr)} — caller blame`;
			bodyStmts.push(
				array(
					sym("if"),
					array(sym("!"), preExpr),
					array(
						sym("throw"),
						array(sym("new"), sym("Error"), { type: "string", value: preMsg }),
					),
				),
			);
		}

		// Instrument yields if :yields type is specified and not :any
		let instrumentedBody = bodyClause;
		if (yieldsClause && yieldsClause.length > 0) {
			const yieldsType = yieldsClause[0];
			if (isKeyword(yieldsType) && yieldsType.value !== "any") {
				instrumentedBody = bodyClause.map((expr) =>
					instrumentYields(expr, yieldsType, funcName),
				);
			}
		}

		bodyStmts.push(...instrumentedBody);

		return array(
			sym("function*"),
			funcNameNode,
			array(...pNames),
			...bodyStmts,
		);
	});

	// (genfn (params) :yields :type body...)
	// or (genfn (params) body...) — no yield check
	const genfnMacro = (...args) => {
		if (args.length < 2) {
			throw new Error("genfn requires at least a parameter list and body");
		}
		const paramList = args[0];
		if (!isArray(paramList)) {
			throw new Error("genfn: first argument must be a parameter list");
		}

		// Check for :yields keyword after param list
		let yieldsType = null;
		let bodyStart = 1;
		if (args.length >= 3 && isKeyword(args[1]) && args[1].value === "yields") {
			if (args.length < 4) {
				throw new Error("genfn: :yields requires a type keyword and body");
			}
			yieldsType = args[2];
			bodyStart = 3;
		}

		const bodyForms = args.slice(bodyStart);
		const params = parseTypedParams(paramList);
		const pNames = params.flatMap((p) => paramNameNodes(p));

		// Type checks
		const typeChecks = [];
		for (const p of params) {
			typeChecks.push(...paramTypeChecks(p, "anonymous"));
		}

		// Instrument yields
		let instrumentedBody = bodyForms;
		if (yieldsType && isKeyword(yieldsType) && yieldsType.value !== "any") {
			instrumentedBody = bodyForms.map((expr) =>
				instrumentYields(expr, yieldsType, "anonymous"),
			);
		}

		// Build generator body — similar to fn but with function*
		// function* generators always need block body
		const allBody = [...typeChecks, ...instrumentedBody];

		// Anonymous function* expression — pass param list directly (no name)
		return array(
			sym("function*"),
			array(...pNames),
			...allBody,
		);
	};

	macroEnv.set("genfn", genfnMacro);

	// --- type ---
	// (type Option (Some :any value) None)
	macroEnv.set("type", (...args) => {
		if (args.length < 2) {
			throw new Error("type requires a name and at least one constructor");
		}
		const typeName = args[0];
		if (typeName.type !== "atom") {
			throw new Error("type: first argument must be a type name");
		}

		const constructors = args.slice(1);
		const forms = [];

		for (const ctor of constructors) {
			if (ctor.type === "atom") {
				// Zero-field constructor: (const None (object (tag "None")))
				const ctorName = ctor.value;
				typeRegistry.set(ctorName, []);
				forms.push(
					array(
						sym("const"),
						ctor,
						array(
							sym("object"),
							array(sym("tag"), { type: "string", value: ctorName }),
						),
					),
				);
			} else if (isArray(ctor) && ctor.values.length >= 1) {
				// Constructor with fields: (function Some (value) <checks> (return (object ...)))
				const ctorName = ctor.values[0].value;
				const fields = parseTypedParams({
					type: "list",
					values: ctor.values.slice(1),
				});
				const fieldNames = fields.map((f) => f.name.value);
				typeRegistry.set(ctorName, fieldNames);

				const paramNames = fields.map((f) => f.name);
				const typeChecks = [];
				for (const f of fields) {
					const check = buildTypeCheck(f.name, f.typeKw, ctorName, "field");
					if (check) typeChecks.push(check);
				}

				const objPairs = [
					array(sym("tag"), { type: "string", value: ctorName }),
				];
				for (const f of fields) {
					objPairs.push(array(sym(f.name.value), f.name));
				}

				forms.push(
					array(
						sym("function"),
						sym(ctorName),
						array(...paramNames),
						...typeChecks,
						array(sym("return"), array(sym("object"), ...objPairs)),
					),
				);
			}
		}

		if (forms.length === 1) return forms[0];
		return forms;
	});

	// --- func ---
	// Single clause: (func name :args (:type a) :returns :type :pre expr :post expr :body expr)
	// Multi-clause: (func name (:args ... :body ...) (:args ... :body ...))
	// Zero-arg: (func name body-expr...)
	macroEnv.set("func", (...args) => {
		if (args.length < 2) {
			throw new Error("func requires at least a name and body");
		}
		const funcNameNode = args[0];
		if (funcNameNode.type !== "atom") {
			throw new Error("func: first argument must be a function name");
		}
		const funcName = funcNameNode.value;
		const restArgs = args.slice(1);

		// Detect mode: multi-clause, single-clause, or zero-arg
		const firstAfterName = restArgs[0];

		// Multi-clause: first arg is a list whose first element is a keyword
		if (
			isArray(firstAfterName) &&
			firstAfterName.values.length > 0 &&
			isKeyword(firstAfterName.values[0])
		) {
			return buildMultiClauseFunc(funcName, funcNameNode, restArgs);
		}

		// Single-clause: first arg is a keyword (:args, :body, etc.)
		if (isKeyword(firstAfterName)) {
			return buildSingleClauseFunc(funcName, funcNameNode, restArgs);
		}

		// Zero-arg shorthand: (func name body-exprs...)
		// Last expression is implicit return
		const bodyForms = restArgs;
		return array(
			sym("function"),
			funcNameNode,
			array(),
			...wrapReturnLast(bodyForms),
		);
	});

	function buildSingleClauseFunc(funcName, funcNameNode, clauseArgs) {
		const clauses = parseKeywordClauses(clauseArgs);
		const argsClause = clauses.get("args");
		const returnsClause = clauses.get("returns");
		const preClause = clauses.get("pre");
		const postClause = clauses.get("post");
		const bodyClause = clauses.get("body");

		if (!bodyClause || bodyClause.length === 0) {
			throw new Error(`func ${funcName}: :body is required`);
		}

		// Parse params
		let params = [];
		if (argsClause && argsClause.length === 1 && isArray(argsClause[0])) {
			params = parseTypedParams(argsClause[0]);
		}
		const pNames = params.flatMap((p) => paramNameNodes(p));

		// Build function body statements
		const bodyStmts = [];

		// Type checks for params
		for (const p of params) {
			bodyStmts.push(...paramTypeChecks(p, funcName));
		}

		// Pre-condition
		if (preClause && preClause.length > 0) {
			const preExpr = preClause[0];
			const preMsg = `${funcName}: pre-condition failed: ${formatSExpr(preExpr)} — caller blame`;
			bodyStmts.push(
				array(
					sym("if"),
					array(sym("!"), preExpr),
					array(
						sym("throw"),
						array(sym("new"), sym("Error"), { type: "string", value: preMsg }),
					),
				),
			);
		}

		// Determine return behavior
		const hasReturns = returnsClause && returnsClause.length > 0;
		const returnsType = hasReturns ? returnsClause[0] : null;
		const isVoid =
			returnsType && isKeyword(returnsType) && returnsType.value === "void";
		const hasPost = postClause && postClause.length > 0;

		if (hasPost) {
			const resultVar = gensym("result");
			// Body: capture result
			if (bodyClause.length === 1) {
				bodyStmts.push(array(sym("const"), resultVar, bodyClause[0]));
			} else {
				// Multiple body exprs — last one is the value
				const initBody = bodyClause.slice(0, -1);
				bodyStmts.push(...initBody);
				bodyStmts.push(
					array(sym("const"), resultVar, bodyClause[bodyClause.length - 1]),
				);
			}

			// Returns type check on result
			if (hasReturns && !isVoid && returnsType.value !== "any") {
				const retCheck = buildTypeCheck(
					resultVar,
					returnsType,
					funcName,
					"return",
				);
				if (retCheck) bodyStmts.push(retCheck);
			}

			// Post-condition
			const postExpr = postClause[0];
			const postMsg = `${funcName}: post-condition failed: ${formatSExpr(postExpr)} — callee blame`;
			const postWithResult = replaceTilde(postExpr, resultVar);
			bodyStmts.push(
				array(
					sym("if"),
					array(sym("!"), postWithResult),
					array(
						sym("throw"),
						array(sym("new"), sym("Error"), { type: "string", value: postMsg }),
					),
				),
			);

			bodyStmts.push(array(sym("return"), resultVar));
		} else if (hasReturns && !isVoid) {
			// Returns type check
			if (returnsType.value !== "any") {
				const resultVar = gensym("result");
				if (bodyClause.length === 1) {
					bodyStmts.push(array(sym("const"), resultVar, bodyClause[0]));
				} else {
					const initBody = bodyClause.slice(0, -1);
					bodyStmts.push(...initBody);
					bodyStmts.push(
						array(sym("const"), resultVar, bodyClause[bodyClause.length - 1]),
					);
				}
				const retCheck = buildTypeCheck(
					resultVar,
					returnsType,
					funcName,
					"return",
				);
				if (retCheck) bodyStmts.push(retCheck);
				bodyStmts.push(array(sym("return"), resultVar));
			} else {
				// :any return — no check
				bodyStmts.push(...wrapReturnLast(bodyClause));
			}
		} else if (isVoid) {
			bodyStmts.push(...bodyClause);
		} else {
			// No :returns — treat body forms as statements, implicit return of last
			bodyStmts.push(...wrapReturnLast(bodyClause));
		}

		return array(
			sym("function"),
			funcNameNode,
			array(...pNames),
			...bodyStmts,
		);
	}

	function buildMultiClauseFunc(funcName, funcNameNode, clauseLists) {
		const argsVar = gensym("args");
		const stmts = [];

		// Sort clauses: longer arity first, then more typed before less typed
		const parsed = clauseLists.map((cl) => {
			const clauses = parseKeywordClauses(cl.values);
			const argsClause = clauses.get("args");
			let params = [];
			if (argsClause && argsClause.length === 1 && isArray(argsClause[0])) {
				params = parseTypedParams(argsClause[0]);
			}
			const typedCount = params.filter((p) => paramDispatchType(p) !== "any").length;
			return { clauses, params, typedCount, arity: params.length };
		});

		parsed.sort((a, b) => {
			if (a.arity !== b.arity) return b.arity - a.arity;
			return b.typedCount - a.typedCount;
		});

		for (const clause of parsed) {
			const { clauses, params } = clause;
			const returnsClause = clauses.get("returns");
			const preClause = clauses.get("pre");
			const postClause = clauses.get("post");
			const bodyClause = clauses.get("body");

			if (!bodyClause || bodyClause.length === 0) {
				throw new Error(`func ${funcName}: :body is required in each clause`);
			}

			// Build dispatch condition: args.length === N && type checks
			const conditions = [
				array(sym("==="), sym(`${argsVar.value}:length`), {
					type: "number",
					value: params.length,
				}),
			];

			for (let i = 0; i < params.length; i++) {
				const p = params[i];
				const dtype = paramDispatchType(p);
				if (dtype === "any") continue;
				const argAccess = array(sym("get"), argsVar, {
					type: "number",
					value: i,
				});
				// Inline type check for dispatch
				switch (dtype) {
					case "number":
						conditions.push(
							array(sym("==="), array(sym("typeof"), argAccess), {
								type: "string",
								value: "number",
							}),
						);
						break;
					case "string":
						conditions.push(
							array(sym("==="), array(sym("typeof"), argAccess), {
								type: "string",
								value: "string",
							}),
						);
						break;
					case "boolean":
						conditions.push(
							array(sym("==="), array(sym("typeof"), argAccess), {
								type: "string",
								value: "boolean",
							}),
						);
						break;
					case "function":
						conditions.push(
							array(sym("==="), array(sym("typeof"), argAccess), {
								type: "string",
								value: "function",
							}),
						);
						break;
					case "object":
						conditions.push(
							array(
								sym("&&"),
								array(sym("==="), array(sym("typeof"), argAccess), {
									type: "string",
									value: "object",
								}),
								array(sym("!=="), argAccess, sym("null")),
							),
						);
						break;
					case "array":
						conditions.push(array(sym("Array:isArray"), argAccess));
						break;
					default:
						break;
				}
			}

			const condition = andChain(conditions);

			// Build clause body
			const clauseBody = [];

			// Bind params from args
			for (let i = 0; i < params.length; i++) {
				const p = params[i];
				const argAccess = array(sym("get"), argsVar, {
					type: "number",
					value: i,
				});
				if (p.destructured) {
					// const (object name1 name2) = get(args, i)
					clauseBody.push(
						array(sym("const"), paramNameNodes(p)[0], argAccess),
					);
				} else {
					clauseBody.push(
						array(sym("const"), p.name, argAccess),
					);
				}
			}

			// Full type checks (with NaN exclusion etc.)
			for (const p of params) {
				clauseBody.push(...paramTypeChecks(p, funcName));
			}

			// Pre-condition
			if (preClause && preClause.length > 0) {
				const preExpr = preClause[0];
				const preMsg = `${funcName}: pre-condition failed: ${formatSExpr(preExpr)} — caller blame`;
				clauseBody.push(
					array(
						sym("if"),
						array(sym("!"), preExpr),
						array(
							sym("throw"),
							array(sym("new"), sym("Error"), {
								type: "string",
								value: preMsg,
							}),
						),
					),
				);
			}

			// Body + return
			const hasReturns = returnsClause && returnsClause.length > 0;
			const hasPost = postClause && postClause.length > 0;

			if (hasPost) {
				const resultVar = gensym("result");
				if (bodyClause.length === 1) {
					clauseBody.push(array(sym("const"), resultVar, bodyClause[0]));
				} else {
					clauseBody.push(...bodyClause.slice(0, -1));
					clauseBody.push(
						array(sym("const"), resultVar, bodyClause[bodyClause.length - 1]),
					);
				}
				const postExpr = postClause[0];
				const postMsg = `${funcName}: post-condition failed: ${formatSExpr(postExpr)} — callee blame`;
				const postWithResult = replaceTilde(postExpr, resultVar);
				clauseBody.push(
					array(
						sym("if"),
						array(sym("!"), postWithResult),
						array(
							sym("throw"),
							array(sym("new"), sym("Error"), {
								type: "string",
								value: postMsg,
							}),
						),
					),
				);
				clauseBody.push(array(sym("return"), resultVar));
			} else if (hasReturns) {
				clauseBody.push(...wrapReturnLast(bodyClause));
			} else {
				clauseBody.push(...bodyClause);
			}

			stmts.push(
				array(sym("if"), condition, array(sym("block"), ...clauseBody)),
			);
		}

		// Final throw for no matching clause
		stmts.push(
			array(
				sym("throw"),
				array(sym("new"), sym("TypeError"), {
					type: "string",
					value: `${funcName}: no matching clause for arguments`,
				}),
			),
		);

		return array(
			sym("function"),
			funcNameNode,
			array(array(sym("rest"), argsVar)),
			...stmts,
		);
	}

	// --- match ---
	// (match expr (pattern body) (pattern :when guard body) (_ default))
	// Always wraps in IIFE
	macroEnv.set("match", (...args) => {
		if (args.length < 2) {
			throw new Error("match requires an expression and at least one clause");
		}
		const expr = args[0];
		const clauses = args.slice(1);
		const targetVar = gensym("target");
		const stmts = [array(sym("const"), targetVar, expr)];

		for (let i = 0; i < clauses.length; i++) {
			const clause = clauses[i];
			if (!isArray(clause) || clause.values.length < 2) {
				throw new Error(
					`match: clause ${i} must be (pattern body...) or (pattern :when guard body...)`,
				);
			}

			const pattern = clause.values[0];
			let guard = null;
			let bodyStart = 1;

			// Check for :when guard
			if (
				clause.values.length >= 3 &&
				isKeyword(clause.values[1]) &&
				clause.values[1].value === "when"
			) {
				guard = clause.values[2];
				bodyStart = 3;
			}

			const bodyForms = clause.values.slice(bodyStart);
			if (bodyForms.length === 0) {
				throw new Error(`match: clause ${i} has no body`);
			}

			const { checks, bindings } = compilePattern(pattern, targetVar);

			// Add guard to checks
			if (guard) {
				// Guard may reference bound variables — we need bindings before guard eval
				// So for guarded patterns, put check in if, bindings inside, then guard check
				const condition = checks.length > 0 ? andChain(checks) : null;
				const innerBlock = [...bindings];

				// Guard check with nested if
				const wrapped = wrapReturnLast(bodyForms);
				const guardedBody =
					wrapped.length === 1
						? wrapped[0]
						: array(sym("block"), ...wrapped);

				innerBlock.push(array(sym("if"), guard, guardedBody));

				if (condition) {
					stmts.push(
						array(sym("if"), condition, array(sym("block"), ...innerBlock)),
					);
				} else {
					stmts.push(array(sym("block"), ...innerBlock));
				}
			} else {
				// No guard — simple case
				const isWildcard = pattern.type === "atom" && pattern.value === "_";
				const isSimpleBinding =
					pattern.type === "atom" &&
					!isPascalCase(pattern.value) &&
					pattern.value !== "_" &&
					pattern.value !== "true" &&
					pattern.value !== "false" &&
					pattern.value !== "null" &&
					pattern.value !== "undefined";

				if (isWildcard || isSimpleBinding) {
					// Default / catch-all — no condition check
					const block = [...bindings, ...wrapReturnLast(bodyForms)];
					stmts.push(array(sym("block"), ...block));
				} else {
					const condition = andChain(checks);
					const block = [...bindings, ...wrapReturnLast(bodyForms)];
					stmts.push(
						array(sym("if"), condition, array(sym("block"), ...block)),
					);
				}
			}
		}

		// If last clause is not a wildcard/binding, add throw
		const lastClause = clauses[clauses.length - 1];
		const lastPattern = lastClause.values[0];
		const isLastWildcard =
			lastPattern.type === "atom" && lastPattern.value === "_";
		const isLastBinding =
			lastPattern.type === "atom" &&
			!isPascalCase(lastPattern.value) &&
			lastPattern.value !== "true" &&
			lastPattern.value !== "false" &&
			lastPattern.value !== "null" &&
			lastPattern.value !== "undefined";

		if (!isLastWildcard && !isLastBinding) {
			stmts.push(
				array(
					sym("throw"),
					array(sym("new"), sym("Error"), {
						type: "string",
						value: "match: no matching pattern",
					}),
				),
			);
		}

		// Wrap in IIFE
		const arrowFn = array(sym("=>"), array(), ...stmts);
		return array(arrowFn);
	});

	// --- some-> (nil-safe thread-first) ---
	// (some-> x (f a) (g b)) → IIFE with null checks
	macroEnv.set("some->", (...args) => buildSomeThread(args, "first"));

	// --- some->> (nil-safe thread-last) ---
	macroEnv.set("some->>", (...args) => buildSomeThread(args, "last"));

	// --- if-let ---
	// (if-let (pattern expr) then else)
	// (if-let ((Some v) expr) then else)
	// (if-let ((obj :key v) expr) then else)
	// Always IIFE
	macroEnv.set("if-let", (...args) => {
		if (args.length < 2 || args.length > 3) {
			throw new Error(
				"if-let requires 2-3 arguments: (if-let (binding expr) then else?)",
			);
		}
		const bindingPair = args[0];
		const thenBody = args[1];
		const elseBody = args.length === 3 ? args[2] : null;

		if (!isArray(bindingPair) || bindingPair.values.length !== 2) {
			throw new Error("if-let: first argument must be (pattern expr)");
		}

		const pattern = bindingPair.values[0];
		const expr = bindingPair.values[1];
		const tempVar = gensym("t");

		const stmts = [array(sym("const"), tempVar, expr)];

		const result = compileLetPattern(pattern, tempVar);
		if (!result) {
			throw new Error(`if-let: unrecognized pattern: ${formatSExpr(pattern)}`);
		}

		const { condition, bindings } = result;
		const thenBlock = [...bindings, array(sym("return"), thenBody)];
		if (elseBody) {
			stmts.push(
				array(
					sym("if"),
					condition,
					array(sym("block"), ...thenBlock),
					array(sym("block"), array(sym("return"), elseBody)),
				),
			);
		} else {
			stmts.push(
				array(sym("if"), condition, array(sym("block"), ...thenBlock)),
			);
		}

		const arrowFn = array(sym("=>"), array(), ...stmts);
		return array(arrowFn);
	});

	// --- when-let ---
	// (when-let (pattern expr) body...)
	// Same as if-let but no else branch
	macroEnv.set("when-let", (...args) => {
		if (args.length < 2) {
			throw new Error(
				"when-let requires at least 2 arguments: (when-let (binding expr) body...)",
			);
		}
		const bindingPair = args[0];
		const bodyForms = args.slice(1);

		if (!isArray(bindingPair) || bindingPair.values.length !== 2) {
			throw new Error("when-let: first argument must be (pattern expr)");
		}

		const pattern = bindingPair.values[0];
		const expr = bindingPair.values[1];
		const tempVar = gensym("t");

		const stmts = [array(sym("const"), tempVar, expr)];

		const result = compileLetPattern(pattern, tempVar);
		if (!result) {
			throw new Error(
				`when-let: unrecognized pattern: ${formatSExpr(pattern)}`,
			);
		}

		const { condition, bindings } = result;
		const wrapped = wrapReturnLast(bodyForms);
		const returnBody =
			wrapped.length === 1
				? wrapped[0]
				: array(sym("block"), ...wrapped);

		stmts.push(
			array(
				sym("if"),
				condition,
				array(sym("block"), ...bindings, returnBody),
			),
		);

		const arrowFn = array(sym("=>"), array(), ...stmts);
		return array(arrowFn);
	});
}
