# Error Handling

Essential patterns for throwing, catching, and propagating errors in
lykn. Covers synchronous exceptions, Promise rejections, async/await
error flows, custom error types, contracts, and validation discipline —
leveraging lykn's surface language for safer error handling than raw
JavaScript allows.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: Always Throw `Error` Objects, Never Strings or Plain Values

**Strength**: MUST

**Summary**: Only `Error` instances carry `:stack`, `:message`, `:name`,
and `:cause` — throw anything else and you lose all of them.

```lykn
;; Good
(throw (new Error "Connection refused"))
(throw (new TypeError "Expected a number, got string"))
(throw (new RangeError (template "Index " i " out of bounds [0, " max ")")))

;; Bad — no stack trace, no .cause support, no .name
(throw "Connection refused")
(throw 404)
```

**Rationale**: Stack traces are captured at `(new Error)` creation time.
Throwing a string or plain object loses the call stack, making debugging
a guessing game.

---

## ID-02: Use `throw` for Exceptional Conditions, Not Control Flow

**Strength**: MUST

**Summary**: Throw when something unexpected has happened. Do not use
exceptions for expected outcomes like "user not found."

```lykn
;; Good — exceptional: invalid argument, missing required field
(func parse-config
  :args (:string path)
  :returns :object
  :body
  (bind raw (Deno:readTextFileSync path))
  (bind config (JSON:parse raw))
  (if (not config:version)
    (throw (new Error (template "Config at " path " missing required \"version\" field"))))
  config)

;; Bad — throwing for expected "not found" flow
(func find-user :args (:string id) :returns :any :body
  (bind user (users:get id))
  (if (not user) (throw (new Error "User not found")))
  user)

;; Good — return undefined for expected absence
(func find-user :args (:string id) :returns :any :body
  (users:get id))
```

**Rationale**: Exceptions unwind the call stack and disrupt normal
control flow. Using them for expected outcomes forces callers into
`try`/`catch` for routine logic and obscures genuine errors.

**See also**: `02-api-design.md` ID-23, ID-24

---

## ID-03: Include Context in Error Messages

**Strength**: MUST

**Summary**: Error messages must say what failed and why. Include the
relevant values using `template`.

```lykn
;; Good — says what, why, and includes the offending value
(throw (new TypeError
  (template "Expected port to be a number, got " (typeof port) ": " port)))
(throw (new RangeError
  (template "Retry count " retries " exceeds maximum (" MAX-RETRIES ")")))
(throw (new Error
  (template "Failed to fetch " url ": " response:status " " response:status-text)))

;; Bad — says nothing useful
(throw (new Error "Invalid argument"))
(throw (new Error "Something went wrong"))
```

**Rationale**: When an error surfaces in logs or a stack trace, the
message is often the only context. A message with the value and
constraint makes the fix obvious.

---

## ID-04: Use `Error:cause` for Error Chaining

**Strength**: SHOULD

**Summary**: When catching and rethrowing, pass the original error as
`(obj :cause err)` to preserve the full chain.

```lykn
;; Good — chain preserves the original error and its stack trace
(async (func load-config
  :args (:string path)
  :returns :object
  :body
  (try
    (bind raw (await (Deno:readTextFile path)))
    (JSON:parse raw)
    (catch err
      (throw (new Error
        (template "Failed to load config from " path)
        (obj :cause err)))))))
```

Compiles to:

```js
async function loadConfig(path) {
  /* type check ... */
  try {
    const raw = await Deno.readTextFile(path);
    return JSON.parse(raw);
  } catch (err) {
    throw new Error(`Failed to load config from ${path}`, {cause: err});
  }
}
```

**Rationale**: `Error:cause` (ES2022) preserves the original error and
its stack trace while adding higher-level context. Without it,
rethrowing loses the root cause.

---

## ID-05: Subclass `Error` for Domain-Specific Error Types

**Strength**: SHOULD

**Summary**: Create custom error classes for errors that callers need
to handle differently from generic errors.

```lykn
;; Good — custom error with context properties
(class HttpError (Error)
  (constructor ((status) (status-text) (url))
    (super (template status " " status-text ": " url))
    (= this:status status)
    (= this:status-text status-text)
    (= this:url url))
  (get name () (return "HttpError")))

(class ValidationError (Error)
  (constructor ((field) (reason))
    (super (template "Validation failed for \"" field "\": " reason))
    (= this:field field)
    (= this:reason reason))
  (get name () (return "ValidationError")))
```

```lykn
;; Usage — selective catch
(try
  (await (submit-form data))
  (catch err
    (if (instanceof err ValidationError)
      (show-field-error err:field err:reason)
      (if (and (instanceof err HttpError) (= err:status 429))
        (await (retry-after-delay))
        (throw err)))))
```

**Rationale**: Custom error classes enable `instanceof` checks for
selective catching, carry domain-specific properties, and produce
descriptive `:name` values in logs.

---

## ID-06: Custom Errors Must Set `name`

**Strength**: MUST

**Summary**: Override `:name` in custom error classes so logs and stack
traces identify the error type.

```lykn
;; Good — name matches the class
(class ConfigError (Error)
  (get name () (return "ConfigError")))

;; Bad — name defaults to "Error" (misleading in logs)
(class ConfigError (Error))
```

**Rationale**: Without a custom `:name`, all subclasses display as
"Error" in stack traces and `console:error` output.

---

## ID-07: Use `AggregateError` for Multiple Simultaneous Failures

**Strength**: CONSIDER

**Summary**: When multiple independent operations can fail and you need
all the failure reasons, use `AggregateError`.

```lykn
;; Good — collect all validation errors at once
(func validate-form :args (:object data) :returns :object :body
  (bind errors (cell #a()))
  (if (not data:name)
    (swap! errors (fn (:array e) (conj e (new Error "name is required")))))
  (if (not data:email)
    (swap! errors (fn (:array e) (conj e (new Error "email is required")))))
  (if (and (!= data:age undefined) (< data:age 0))
    (swap! errors (fn (:array e) (conj e (new RangeError "age must be non-negative")))))
  (if (> (express errors):length 0)
    (throw (new AggregateError (express errors) "Form validation failed")))
  data)
```

**Rationale**: `AggregateError` (ES2021) holds an array of errors in
`:errors`. It is the rejection value of `Promise:any` when all inputs
reject, and is appropriate for batch validation.

---

## ID-08: Never Write Empty `catch` Blocks

**Strength**: MUST

**Summary**: An empty `catch` silently discards both expected and
unexpected errors. Always log, handle, or rethrow.

```lykn
;; Bad — swallows everything
(try (risky-operation) (catch e))

;; Good — handle known errors, rethrow unknown
(try
  (risky-operation)
  (catch e
    (if (instanceof e KnownError)
      (recover e)
      (throw e))))

;; Good — log if you can't handle
(try
  (risky-operation)
  (catch e
    (console:error "risky-operation failed:" e)
    (throw e)))
```

**Rationale**: Empty catch blocks are the root cause of "the program
does nothing and nobody knows why" bugs. Typos, wrong types, and logic
errors become completely invisible.

---

## ID-09: Catch Specific Errors, Not Everything

**Strength**: SHOULD

**Summary**: Use `instanceof` checks inside `catch` to handle only
errors you understand. Rethrow everything else.

```lykn
;; Good — selective catching
(try
  (bind data (JSON:parse raw))
  (process-data data)
  (catch e
    (if (instanceof e SyntaxError)
      (obj :error "Invalid JSON" :raw raw)
      (throw e))))
```

**Rationale**: JavaScript's single `catch` clause catches all
exceptions without discrimination. Without `instanceof` filtering,
programmer errors are silently swallowed.

---

## ID-10: Re-Throw Unknown Errors

**Strength**: MUST

**Summary**: After handling known error types, always `throw` the error
if it's not one you recognize.

```lykn
;; Good — handle known, rethrow unknown
(try
  (await (fetch-resource url))
  (catch e
    (if (and (instanceof e HttpError) (= e:status 404))
      null
      (if (and (instanceof e HttpError) (= e:status 429))
        (block
          (await (delay 1000))
          (fetch-resource url))
        (throw e)))))
```

**Rationale**: Catching and not rethrowing converts an error into
silent success. Unknown errors should propagate to a top-level handler.

---

## ID-11: Use `finally` for Cleanup, Not for Return Values

**Strength**: SHOULD

**Summary**: `finally` always runs — use it for cleanup. Never return
from a `finally` block.

```lykn
;; Good — resource cleanup
(bind file (await (Deno:open "data.txt")))
(try
  (await (process-file file))
  (finally (file:close)))

;; Bad — return in finally silently overrides try's throw
;; The error vanishes!
```

**Rationale**: `return`, `throw`, or `break` inside `finally` overrides
any pending result from `try` or `catch`. This can silently discard
exceptions. Use `finally` exclusively for cleanup.

---

## ID-12: Omit the Catch Binding When You Don't Need It

**Strength**: CONSIDER

**Summary**: When you only need to know that an error occurred, not
what it was, use `catch` without a parameter.

```lykn
;; Good — parse with fallback, error details irrelevant
(func try-parse-json :args (:string s) :returns :any :body
  (try (JSON:parse s) (catch undefined)))

;; Good — boolean "does it throw?" check
(func valid-json? :args (:string s) :returns :boolean :body
  (try (block (JSON:parse s) true) (catch false)))
```

**Rationale**: Omitting the catch binding signals to readers that the
error value is intentionally unused — not a license for empty catch
blocks.

---

## ID-13: Always Handle Promise Rejections

**Strength**: MUST

**Summary**: Every Promise chain must have a rejection handler.
Unhandled rejections terminate Deno.

```lykn
;; Good — async/await with try/catch
(async (func run :returns :void :body
  (try
    (bind data (await (fetch-data url)))
    (bind result (await (process data)))
    (await (save result))
    (catch err
      (console:error "Pipeline failed:" err)))))

;; Bad — dangling Promise, rejection lost
(fetch-data url)
```

**Rationale**: In Deno, unhandled rejections terminate the process.
Every Promise chain must end with either `:catch` or be inside a
`try`/`catch` with `await`.

---

## ID-14: Prefer `async`/`await` with `try`/`catch`

**Strength**: SHOULD

**Summary**: Use `async`/`await` for async error handling. It reads
linearly, supports loops and conditionals, and uses the same
`try`/`catch` as synchronous code.

```lykn
;; Good — linear, readable, standard error handling
(async (func load-user
  :args (:string id)
  :returns :any
  :body
  (try
    (bind response (await (fetch (template "/api/users/" id))))
    (if (not response:ok)
      (throw (new HttpError response:status response:status-text response:url)))
    (await (response:json))
    (catch err
      (if (and (instanceof err HttpError) (= err:status 404))
        null
        (throw err))))))
```

**Rationale**: `async`/`await` supports native loops, conditionals,
and `try`/`catch` — constructs that are awkward in Promise chains.

---

## ID-15: Understand `:catch` Placement — It Matters

**Strength**: MUST

**Summary**: In Promise chains, `:catch` only handles rejections from
earlier in the chain. Placement determines scope.

```lykn
;; Good — terminal catch covers the entire chain
(-> (fetch-data)
  (:then (fn (:any data) (transform data)))
  (:then (fn (:any result) (save result)))
  (:catch (fn (:any err) (handle-error err))))
```

**Rationale**: Using `:then` with both success and error arguments
will NOT catch exceptions thrown by the success handler itself. A
separate `:catch` avoids this gap.

---

## ID-16: Use `Promise:all-settled` When Some Failures Are Acceptable

**Strength**: SHOULD

**Summary**: When you need all outcomes regardless of individual
failures, use `Promise:allSettled` instead of `Promise:all`.

```lykn
;; Good — inspect each outcome independently
(bind results (await (Promise:allSettled #a(
  (fetch "/api/users")
  (fetch "/api/posts")
  (fetch "/api/comments")))))

(for-of result results
  (if (= result:status "fulfilled")
    (process-response result:value)
    (console:warn "Request failed:" result:reason)))
```

**Rationale**: `Promise:all` short-circuits on the first rejection.
`Promise:allSettled` (ES2020) never rejects; it returns an array of
`(obj :status :value/:reason)` objects for every input.

---

## ID-17: Use `Promise:any` for First-Success Patterns

**Strength**: CONSIDER

**Summary**: `Promise:any` fulfills with the first success, ignoring
individual rejections. It only rejects when all inputs fail.

```lykn
;; Good — try multiple sources, take the first success
(bind content (await (Promise:any #a(
  (-> (fetch "https://cdn1.example.com/data.json") (:then (fn (:any r) (r:text))))
  (-> (fetch "https://cdn2.example.com/data.json") (:then (fn (:any r) (r:text))))))))

;; Handle total failure — err is AggregateError
(try
  (bind content (await (Promise:any sources)))
  (catch err
    (console:error "All sources failed:")
    (for-of e err:errors
      (console:error (template "  - " e:message)))))
```

**Rationale**: `Promise:any` (ES2021) needs just one success. The
rejection value is an `AggregateError` containing all individual
rejection reasons.

---

## ID-18: Never Mix Callbacks and Promises

**Strength**: MUST

**Summary**: A function must be either callback-based or Promise-based,
never both. In new code, always use Promises.

```lykn
;; Good — Promise-only API
(export (async (func read-config
  :args (:string path)
  :returns :object
  :body
  (bind raw (await (Deno:readTextFile path)))
  (JSON:parse raw))))
```

**Critical rule**: Promise-based functions must never throw synchronous
exceptions. Callers set up `:catch` handlers, not `try`/`catch` around
the call.

**See also**: `02-api-design.md` ID-25

---

## ID-19: `return await` Inside `try` — When It Matters

**Strength**: MUST

**Summary**: Inside a `try`/`catch`, use `(await ...)` on the returned
expression to ensure rejections are caught locally.

```lykn
;; Good — await ensures rejection is caught locally
(async (func load-data
  :args (:string url)
  :returns :any
  :body
  (try
    (await (-> (fetch url) (:then (fn (:any r) (r:json)))))
    (catch err
      (console:error (template "Failed to load " url ":") err)
      null))))
```

**Rationale**: Without `await`, `return` passes the Promise through
without unwrapping it — the local `catch` is bypassed entirely.

---

## ID-20: Beware Fire-and-Forget Async Calls

**Strength**: MUST

**Summary**: Calling an async function without `await` starts it in
the background. Any rejection is silently lost.

```lykn
;; Bad — rejection has nowhere to go
(async (func handle-request :args (:any req) :returns :any :body
  (log-request req)    ;; async, unawaited — rejection lost
  (bind data (await (get-data)))
  data))

;; Good — await it
(async (func handle-request :args (:any req) :returns :any :body
  (await (log-request req))
  (bind data (await (get-data)))
  data))

;; Good — fire-and-forget with explicit catch
(async (func handle-request :args (:any req) :returns :any :body
  ((log-request req):catch (fn (:any err)
    (console:error "Log failed:" err)))
  (bind data (await (get-data)))
  data))
```

**Rationale**: An unawaited async call creates a floating Promise with
no rejection handler. In Deno, unhandled rejections terminate the
process.

---

## ID-21: Parallel `await` — Use `Promise:all`, Not Sequential Awaits

**Strength**: SHOULD

**Summary**: When async operations are independent, run them in parallel
with `Promise:all`.

```lykn
;; Good — parallel execution
(bind (array users posts) (await (Promise:all #a(
  (fetch-users)
  (fetch-posts)))))

;; Bad — sequential, doubles total wait time
(bind users (await (fetch-users)))
(bind posts (await (fetch-posts)))
```

**Rationale**: Sequential `await` on independent operations is a
performance anti-pattern. `Promise:all` starts all operations
simultaneously.

---

## ID-22: Awaiting Is Shallow — Nested Async Needs `Promise:all`

**Strength**: SHOULD

**Summary**: `await` only pauses its immediately enclosing `async`
function. Nested async callbacks require `Promise:all`.

```lykn
;; Good — await the array of Promises
(async (func process-all :args (:array items) :returns :array :body
  (await (Promise:all
    (items:map (fn (:any item) (transform item)))))))
```

**Rationale**: `:map` with an async function returns `Promise[]`, not
resolved values. Collect the Promises and `(await (Promise:all ...))`.

---

## ID-23: Validate at Boundaries with `func` Contracts

**Strength**: SHOULD

**Summary**: Use `func` with type annotations and `:pre` contracts at
public API entry points. Inside validated boundaries, trust the data.

```lykn
;; Good — func contracts validate at the boundary
(export (func create-user
  :args (:string name :string email)
  :returns :object
  :pre (and (> name:length 0) (email:includes "@"))
  :body (obj :id (generate-id) :name name :email email)))

;; Internal functions trust validated data — no redundant checks
(func format-user-line :args (:string name) :returns :string :body
  (template "User: " name))
```

Compiles to:

```js
export function createUser(name, email) {
  if (typeof name !== "string") throw new TypeError(/* ... */);
  if (typeof email !== "string") throw new TypeError(/* ... */);
  if (!(name.length > 0 && email.includes("@")))
    throw new Error("create-user: pre-condition failed: ...");
  return {id: generateId(), name, email};
}
```

**Rationale**: `func` type annotations and `:pre` contracts replace
manual validation guards. The compiler generates the checks, the error
messages include the contract expression, and `--strip-assertions`
removes them in production. Redundant validation inside internal code
adds noise without safety.

**See also**: `02-api-design.md` ID-27

---

## ID-24: Fail Fast — Detect and Throw on Invalid Input Immediately

**Strength**: MUST

**Summary**: Reject invalid input at the top of a function with guard
clauses or `:pre` contracts. Don't let bad data travel.

```lykn
;; Good — pre-condition catches invalid input immediately
(func fetch-user
  :args (:string id)
  :returns :any
  :pre (> id:length 0)
  :body
  (bind response (await (fetch (template "/api/users/" id))))
  (if (not response:ok)
    (throw (new HttpError response:status response:status-text response:url)))
  (response:json))
```

```lykn
;; Bad — error surfaces far from the cause
;; id is undefined → fetches "/api/users/undefined" → 404 or garbage
```

**Rationale**: JavaScript silently coerces `undefined` and `null` into
strings when interpolated. Failing fast with `:pre` produces an error
with a useful stack trace pointing at the caller.

**See also**: `01-core-idioms.md` ID-26

---

## ID-25: Prefer Returning `undefined`/`null` or `type` for "Not Found"

**Strength**: SHOULD

**Summary**: When absence is an expected outcome, return `undefined` or
use `type` with `Some`/`None`. Reserve `throw` for unexpected failures.

```lykn
;; Good — undefined for "not found"
(func find-user :args (:string id) :returns :any :body
  (users:get id))

;; Better — type makes absence explicit
(type Option (Some :any value) None)

(func find-user :args (:string id) :returns :any :body
  (bind user (users:get id))
  (if (js:eq user null) (None) (Some user)))

;; Caller uses match — can't forget to handle None
(match (find-user "abc")
  ((Some u) (greet u))
  (None (console:log "not found")))
```

**Rationale**: Throwing forces callers into `try`/`catch` for routine
lookups. Returning `undefined` or `Option` lets callers use `??`,
`some->`, or `match`.

**See also**: `02-api-design.md` ID-23, ID-24

---

## ID-26: Use Assertions in Tests, Not in Production Code

**Strength**: SHOULD

**Summary**: `assert` functions throw `AssertionError` and are designed
for tests. In production, use `func` contracts or explicit guards.

```lykn
;; Good — in tests
(import "https://deno.land/std/assert/mod.ts" (assert-equals assert-throws))

(Deno:test "parse-port rejects non-numeric"
  (fn () (assert-throws (fn () (parse-port "abc")) TypeError)))

;; Good — in production (func contract)
(func parse-port
  :args (:string input)
  :returns :number
  :body
  (bind port (Number input))
  (if (or (not (Number:isFinite port)) (< port 0) (> port 65535))
    (throw (new RangeError (template "Invalid port: " input))))
  port)
```

**Rationale**: `assert` messages are generic and conflate test
infrastructure with application logic. In production, throw
domain-appropriate errors with specific messages, or use `:pre`
contracts.

---

## ID-27: Never Swallow Errors Silently

**Strength**: MUST

**Summary**: If you catch an error, you must do something with it —
log, handle, or rethrow. Silent swallowing is always a bug.

```lykn
;; Bad — silent swallow
(try (await (sync-data)) (catch e))

;; Good — explicit handling with structured logging
(try
  (await (sync-data))
  (catch e
    (console:error "[sync] Failed:" e:message (obj :cause e:cause))
    (throw e)))
```

**Rationale**: Every silent swallow creates a class of bugs that are
invisible to developers. Even during prototyping, at minimum log the
error.

---

## ID-28: In `async` Functions, Just `throw` — Not `Promise:reject`

**Strength**: SHOULD

**Summary**: Inside an `async` function, `throw` is the natural way to
reject. `Promise:reject` is redundant.

```lykn
;; Good — throw in async function
(async (func load-data
  :args (:string url)
  :returns :any
  :body
  (bind response (await (fetch url)))
  (if (not response:ok)
    (throw (new HttpError response:status response:status-text url)))
  (response:json)))

;; Bad — unnecessary Promise.reject()
;; (Promise:reject (new HttpError ...))  ;; redundant in async context
```

**Rationale**: In an `async` function, `throw` automatically becomes
a rejected Promise. Using `Promise:reject` adds verbosity without
benefit.

---

## ID-29: Use `match` for Error Dispatch

**Strength**: SHOULD

**Summary**: When handling multiple error types from an operation, use
`type` to model error variants and `match` for exhaustive dispatch.
This is safer than `instanceof` chains.

```lykn
;; Define error variants
(type AppError
  (NotFound :string resource)
  (Forbidden :string reason)
  (ServerError :number status :string message))

;; Dispatch with match — compiler ensures exhaustiveness
(func handle-error :args (:any err) :returns :void :body
  (match err
    ((NotFound r)
      (console:log (template "Not found: " r))
      (show-404))
    ((Forbidden r)
      (console:log (template "Forbidden: " r))
      (show-403))
    ((ServerError s m)
      (console:error (template "Server error " s ": " m))
      (show-500))))
```

**Rationale**: `instanceof` chains are not exhaustive — you can forget
to handle an error type. `match` on a `type` forces you to handle
every variant. If you add a new variant, all `match` expressions must
be updated or they throw at runtime.

**See also**: `01-core-idioms.md` ID-30, `02-api-design.md` ID-04

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | Throw `Error` objects only | MUST | Strings have no `:stack`, `:cause`, `:name` |
| 02 | `throw` for exceptional, not expected | MUST | Don't use exceptions for "not found" |
| 03 | Context in error messages | MUST | Include what failed, why, and the value |
| 04 | `Error:cause` for chaining | SHOULD | Preserves original error when rethrowing |
| 05 | Custom `Error` subclasses | SHOULD | `instanceof` checks + domain properties |
| 06 | Set `:name` on custom errors | MUST | Without it, all display as "Error" |
| 07 | `AggregateError` for multiple failures | CONSIDER | `:errors` array for batch validation |
| 08 | No empty `catch` blocks | MUST | Silent swallowing hides all bugs |
| 09 | Selective catching with `instanceof` | SHOULD | Filter in `catch`, rethrow unknown |
| 10 | Re-throw unknown errors | MUST | Don't convert unknown errors into success |
| 11 | `finally` for cleanup only | SHOULD | `return` in `finally` discards exceptions |
| 12 | Omit catch binding when unused | CONSIDER | Signals intentional discard |
| 13 | Always handle rejections | MUST | Unhandled rejections terminate Deno |
| 14 | `async`/`await` over `:then`/`:catch` | SHOULD | Linear flow, unified error handling |
| 15 | `:catch` placement matters | MUST | Two-arg `:then` misses handler exceptions |
| 16 | `Promise:allSettled` for partial success | SHOULD | Returns all outcomes, never rejects |
| 17 | `Promise:any` for first-success | CONSIDER | Rejects with `AggregateError` when all fail |
| 18 | Never mix callbacks and Promises | MUST | Sync throws escape the Promise chain |
| 19 | `await` inside `try` | MUST | Without `await`, rejection escapes `catch` |
| 20 | Handle fire-and-forget rejections | MUST | Unawaited async calls lose rejections |
| 21 | Parallel `await` with `Promise:all` | SHOULD | Sequential await wastes time |
| 22 | Awaiting is shallow | SHOULD | `:map` with async returns `Promise[]` |
| 23 | `func` contracts at boundaries | SHOULD | `:pre` replaces manual guard clauses |
| 24 | Fail fast with `:pre` | MUST | Catches invalid input at the call site |
| 25 | `type` for modeled absence | SHOULD | `Some`/`None` safer than `null`/`undefined` |
| 26 | Assertions for tests only | SHOULD | Use `:pre` contracts in production |
| 27 | Never swallow errors silently | MUST | Log, handle, or rethrow — never ignore |
| 28 | `throw` in async, not `Promise:reject` | SHOULD | `throw` is the natural rejection |
| 29 | `match` for error dispatch | SHOULD | Exhaustive, no forgotten error types |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for `=` equality (ID-02),
  `??` (ID-03), `some->` (ID-04), early returns (ID-26), `type`+`match`
  (ID-30)
- **API Design**: See `02-api-design.md` for return conventions
  (ID-23, ID-24), contracts (ID-27), consistent types (ID-04)
- **Values & References**: See `04-values-references.md` for mutation
  discipline and defensive copying
- **Type Discipline**: See `05-type-discipline.md` for runtime validation
  and type annotations
- **Async & Concurrency**: See `07-async-concurrency.md` for concurrency
  limits, cancellation, and combinators
- **Anti-Patterns**: See `09-anti-patterns.md` for error handling
  anti-patterns
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  `try`/`catch`/`finally`, `throw`, `type`, `match`
