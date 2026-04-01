/**
 * @module
 * lykn surface form macros.
 * Each macro transforms surface syntax to kernel forms (DD-01 through DD-09).
 * These are the JS reference implementation; the Rust compiler will produce
 * identical expansions as static transforms.
 */

import { sym, array, gensym, isKeyword, isArray } from "./expander.js";

/**
 * Register all surface form macros into the macro environment.
 * @param {Map<string, Function>} macroEnv
 */
export function registerSurfaceMacros(macroEnv) {
	// --- bind ---
	// (bind name value) → (const name value)
	// (bind :type name value) → (const name value)
	macroEnv.set("bind", (...args) => {
		if (args.length < 2) {
			throw new Error("bind requires at least 2 arguments: (bind name value)");
		}
		if (isKeyword(args[0])) {
			// Type annotation — skip it for JS path
			if (args.length < 3) {
				throw new Error(
					"bind with type annotation requires 3 arguments: (bind :type name value)",
				);
			}
			return array(sym("const"), args[1], args[2]);
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
			pairs.push(array(sym(args[i].value), args[i + 1]));
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
		return array(sym("="), cellValue, array(fn, cellValue, ...extraArgs));
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
		return array(sym("="), sym(`${cell.value}:value`), args[1]);
	});

	// --- -> (thread-first) ---
	// (-> x f g) → (g (f x))
	// (-> x (f a) (g b)) → (g (f x a) b)
	macroEnv.set("->", (...args) => {
		if (args.length < 2) {
			throw new Error("-> requires at least 2 arguments: (-> value step...)");
		}
		let threaded = args[0];
		for (let i = 1; i < args.length; i++) {
			const step = args[i];
			if (isArray(step)) {
				// Insert threaded value as second element (after function name)
				const [fn, ...rest] = step.values;
				threaded = array(fn, threaded, ...rest);
			} else {
				// Bare symbol — wrap as single-arg call
				threaded = array(step, threaded);
			}
		}
		return threaded;
	});

	// --- ->> (thread-last) ---
	// (->> x (f a) (g b)) → (g a (f b x))
	macroEnv.set("->>", (...args) => {
		if (args.length < 2) {
			throw new Error("->> requires at least 2 arguments: (->> value step...)");
		}
		let threaded = args[0];
		for (let i = 1; i < args.length; i++) {
			const step = args[i];
			if (isArray(step)) {
				// Insert threaded value as last element
				threaded = array(...step.values, threaded);
			} else {
				// Bare symbol — wrap as single-arg call
				threaded = array(step, threaded);
			}
		}
		return threaded;
	});

	// --- assoc ---
	// (assoc obj :key val) → (object (spread obj) (key val))
	// (assoc obj :a 1 :b 2) → (object (spread obj) (a 1) (b 2))
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
	// (dissoc obj :key) → ((=> () (const (object (alias key _g0) (rest _g1)) obj) _g1))
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
		// IIFE: ((=> () binding restVar))
		const arrowBody = array(sym("=>"), array(), binding, restVar);
		return array(arrowBody);
	});

	// --- conj ---
	// (conj arr val) → (array (spread arr) val)
	macroEnv.set("conj", (...args) => {
		if (args.length !== 2) {
			throw new Error("conj requires exactly 2 arguments: (conj array value)");
		}
		return array(sym("array"), array(sym("spread"), args[0]), args[1]);
	});
}
