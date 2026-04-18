# Async & Concurrency

Concurrency orchestration in lykn: the event loop, Promises,
async/await, async iteration, cancellation, streams, workers, and
scheduling. lykn compiles to JavaScript, so all JS concurrency
semantics apply — single-threaded, run-to-completion, microtask/
macrotask ordering. This guide covers lykn syntax for async patterns.
Async error handling is covered in `03-error-handling.md`.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: JavaScript Is Single-Threaded — Run to Completion

**Strength**: MUST

**Summary**: JavaScript (and therefore lykn's compiled output) executes
one task at a time. Each task finishes before the next begins.

```lykn
;; This runs to completion — no other task can interrupt it
(func handle-request :args (:any data) :returns :any :body
  (bind parsed (JSON:parse data))
  (bind result (transform parsed))
  (serialize result))
```

**Consequence**: No locks or mutexes needed for `cell` state within a
single task. `swap!` is safe — it cannot be interrupted between read
and write.

---

## ID-02: Microtasks Run Before Macrotasks

**Strength**: SHOULD

**Summary**: Promise callbacks (microtasks) drain completely before the
event loop picks up the next timer/IO callback (macrotask).

```lykn
(console:log "1: sync")
(setTimeout (fn () (console:log "4: macrotask")) 0)
(-> (Promise:resolve) (:then (fn () (console:log "2: microtask"))))
(queueMicrotask (fn () (console:log "3: microtask")))
;; Output: 1 → 2 → 3 → 4
```

---

## ID-03: Never Block the Event Loop

**Strength**: MUST

**Summary**: No synchronous I/O, no busy-wait loops, no long
computation on the main thread.

```lykn
;; Good — async file read yields while waiting for I/O
(bind data (await (Deno:readTextFile "large.csv")))

;; Good — promisified delay
(func delay :args (:number ms) :returns :any :body
  (new Promise (fn (:function resolve) (setTimeout resolve ms))))
(await (delay 1000))
```

---

## ID-04: `queueMicrotask` for Immediate Async Scheduling

**Strength**: CONSIDER

**Summary**: Schedule a callback at microtask priority — after current
sync code, before the next macrotask.

```lykn
(queueMicrotask (fn ()
  (console:log "runs before any setTimeout")))
```

---

## ID-05: Promises Start Synchronously, Settle Asynchronously

**Strength**: MUST

**Summary**: The `new Promise` executor runs synchronously. Settlement
notifications are always delivered as microtasks.

```lykn
(console:log "1: before")
(bind p (new Promise (fn (:function resolve)
  (console:log "2: executor (synchronous)")
  (resolve "done"))))
(p:then (fn (:any v) (console:log (template "4: then (" v ")"))))
(console:log "3: after")
;; Output: 1 → 2 → 3 → 4
```

---

## ID-06: Resolving vs Fulfilling — Resolution Can Lock-In

**Strength**: SHOULD

**Summary**: `resolve(value)` fulfills the Promise. `resolve(promise)`
locks-in to that Promise's fate — which may still be pending.

**Key implications**:
- `resolve(Promise:reject(err))` produces a **rejected** Promise
- Returning a Promise from `:then` or `async` flattens automatically
- `reject` does NOT flatten

---

## ID-07: `Promise:withResolvers` for External Settlement Control

**Strength**: SHOULD

**Summary**: ES2024. Breaks `resolve`/`reject` out of the constructor.

```lykn
(bind (object promise resolve reject) (Promise:withResolvers))
(setTimeout (fn () (resolve "delayed value")) 1000)
(bind result (await promise))
```

---

## ID-08: `Promise:try` for Uniform Sync/Async Error Handling

**Strength**: CONSIDER

**Summary**: ES2025. Starts a Promise chain from a callback that may
throw synchronously or return a Promise.

```lykn
(func load-config :args (:string path) :returns :any :body
  (Promise:try (fn ()
    (bind raw (validate-path path))
    (Deno:readTextFile raw))))
```

---

## ID-09: Wrapping Callback APIs — Promisification

**Strength**: SHOULD

**Summary**: Wrap callback-based APIs in `new Promise` for async/await.

```lykn
;; Good — promisified timer
(func delay :args (:number ms) :returns :any :body
  (new Promise (fn (:function resolve) (setTimeout resolve ms))))
(await (delay 1000))

;; Good — promisified event-based API
(func wait-for-connection :args (:any socket) :returns :any :body
  (new Promise (fn (:function resolve :function reject)
    (socket:addEventListener "open" resolve (obj :once true))
    (socket:addEventListener "error" reject (obj :once true)))))
```

---

## ID-10: Top-Level `await` in ESM

**Strength**: SHOULD

**Summary**: Use `await` at the top level of a module without wrapping
in an async function.

```lykn
;; Good — top-level await for module initialization
(bind config (JSON:parse (await (Deno:readTextFile "./config.json"))))
(export (bind db (await (connect-to-database config:db-url))))
```

---

## ID-11: Async Functions Start Synchronously

**Strength**: MUST

**Summary**: Code before the first `await` runs synchronously in the
caller's task.

```lykn
(async (func process :args (:any data) :returns :any :body
  (validate data)                    ;; runs synchronously
  (bind result (await (transform data)))  ;; pauses here
  result))
```

---

## ID-12: Async Infectiousness

**Strength**: SHOULD

**Summary**: An async function returns a Promise. Every caller that
needs the result must be async or handle the Promise explicitly.

```lykn
(async (func get-user :args (:string id) :returns :any :body
  (await (fetch (template "/api/users/" id)))))

(async (func display-user :args (:string id) :returns :void :body
  (bind user (await (get-user id)))
  (render user)))
```

---

## ID-13: `Promise:all` for Parallel Independent Operations

**Strength**: MUST

**Summary**: Start independent async operations concurrently and wait
for all to complete.

```lykn
;; Good — parallel execution
(bind (array users posts) (await (Promise:all #a(
  (-> (fetch "/api/users") (:then (fn (:any r) (r:json))))
  (-> (fetch "/api/posts") (:then (fn (:any r) (r:json))))))))

;; Bad — sequential, doubles total wait time
(bind users (await (-> (fetch "/api/users") (:then (fn (:any r) (r:json))))))
(bind posts (await (-> (fetch "/api/posts") (:then (fn (:any r) (r:json))))))
```

---

## ID-14: `Promise:allSettled` for Partial-Success Scenarios

**Strength**: SHOULD

**Summary**: Wait for all operations to complete, regardless of
individual failures.

```lykn
(bind results (await (Promise:allSettled #a(
  (fetch "/api/users")
  (fetch "/api/posts")
  (fetch "/api/comments")))))

(for-of result results
  (if (= result:status "fulfilled")
    (process-response result:value)
    (console:warn "Failed:" result:reason:message)))
```

**See also**: `03-error-handling.md` ID-16

---

## ID-15: `Promise:any` for First-Success / Redundancy

**Strength**: CONSIDER

**Summary**: Fulfills with the first success. Rejects with
`AggregateError` only when all fail.

```lykn
(bind content (await (Promise:any #a(
  (-> (fetch "https://cdn1.example.com/data.json") (:then (fn (:any r) (r:text))))
  (-> (fetch "https://cdn2.example.com/data.json") (:then (fn (:any r) (r:text))))))))
```

**See also**: `03-error-handling.md` ID-17

---

## ID-16: `Promise:race` for First-Settlement Patterns

**Strength**: SHOULD

**Summary**: Settles with the first Promise to settle — fulfillment or
rejection. For timeouts, prefer `AbortSignal:timeout` (ID-26).

```lykn
;; Good — first response wins
(bind fastest (await (Promise:race #a(
  (fetch "https://api-east.example.com/data")
  (fetch "https://api-west.example.com/data")))))
```

---

## ID-17: Concurrency Limiting — Process N Items at a Time

**Strength**: SHOULD

**Summary**: When processing many items, limit concurrent operations.

```lykn
(async (func map-with-limit
  :args (:array items :number limit :function f)
  :returns :array
  :body
  (bind results (cell #a()))
  (for (let i 0) (< i items:length) (+= i limit)
    (bind batch (items:slice i (+ i limit)))
    (bind batch-results (await (Promise:all (batch:map f))))
    (swap! results (fn (:array r) (r:concat batch-results))))
  (express results)))
```

---

## ID-18: Sequential Async — When Order Matters

**Strength**: SHOULD

**Summary**: Use `for-of` with `await` when each operation depends on
the previous.

```lykn
;; Good — sequential: each step depends on the previous
(async (func migrate-database :args (:array migrations) :returns :void :body
  (for-of migration migrations
    (await (migration:run))
    (await (migration:verify)))))
```

---

## ID-19: `for await...of` for Consuming Async Iterables

**Strength**: SHOULD

**Summary**: `for-await-of` iterates over async iterables.

```lykn
;; Good — consuming a stream
(bind response (await (fetch "/api/stream")))
(for-await-of chunk response:body
  (handle-chunk chunk))
```

---

## ID-20: Async Generators for Producing Async Sequences

**Strength**: SHOULD

**Summary**: `async function*` combines `await` (input) with `yield`
(output).

```lykn
;; Good — async generator for paginated API
(async (function* paginated-fetch (url)
  (bind cursor (cell null))
  (do-while
    (block
      (bind fetch-url (if (express cursor)
        (template url "?cursor=" (express cursor))
        url))
      (bind res (await (fetch fetch-url)))
      (bind data (await (res:json)))
      (yield* data:items)
      (reset! cursor data:next-cursor))
    (express cursor))))
```

---

## ID-21: Async Mapping — `Promise:all(items:map(async fn))`

**Strength**: MUST

**Summary**: `:map` with an async function returns `Promise[]`, not
values. Wrap with `Promise:all`.

```lykn
;; Good — concurrent async map
(bind results (await (Promise:all
  (items:map (async (fn (:any item)
    (bind data (await (fetch-data item:id)))
    (transform data)))))))
```

---

## ID-22: Converting Async Iterables to Arrays

**Strength**: CONSIDER

**Summary**: Use `Array:fromAsync` (ES2024) to collect an async
iterable into an array.

```lykn
(bind all-items (await (Array:fromAsync (paginated-fetch "/api/records"))))
```

---

## ID-23: `AbortController` / `AbortSignal` for Cancellation

**Strength**: SHOULD

**Summary**: Use `AbortController` to cancel async operations.

```lykn
(bind controller (new AbortController))
(bind response (await (fetch "/api/data" (obj :signal controller:signal))))
;; Cancel from elsewhere:
(controller:abort)

;; Good — cleanup pattern
(try
  (bind data (await (fetch url (obj :signal controller:signal))))
  (await (data:json))
  (catch err
    (if (= err:name "AbortError")
      (block (console:log "Request was cancelled") null)
      (throw err))))
```

---

## ID-24: Pass `signal` to Every `fetch` Call

**Strength**: MUST

**Summary**: Every `fetch` in production code should accept a signal.

```lykn
(async (func fetch-data :args (:string url :object opts) :returns :any :body
  (bind response (await (fetch url (obj :signal opts:signal))))
  (if (not response:ok)
    (throw (new Error (template "HTTP " response:status))))
  (response:json)))

;; Caller controls cancellation
(bind controller (new AbortController))
(fetch-data "/api/users" (obj :signal controller:signal))
```

---

## ID-25: Check `signal:aborted` in Long-Running Loops

**Strength**: SHOULD

**Summary**: For long-running async loops, check `signal:aborted` at
each iteration.

```lykn
(async (func process-all :args (:array items :object opts) :returns :array :body
  (bind results (cell #a()))
  (for-of item items
    (if (some-> opts :signal :aborted)
      (throw (new DOMException "Operation cancelled" "AbortError")))
    (swap! results (fn (:array r) (conj r (await (process-item item))))))
  (express results)))
```

---

## ID-26: `AbortSignal:timeout` for Deadline-Based Cancellation

**Strength**: SHOULD

**Summary**: Auto-abort after the specified duration.

```lykn
;; Good — 5-second timeout on fetch
(bind response (await (fetch "/api/slow"
  (obj :signal (AbortSignal:timeout 5000)))))

;; Good — compose with other signals
(bind controller (new AbortController))
(bind signal (AbortSignal:any #a(
  controller:signal
  (AbortSignal:timeout 10000))))
(bind data (await (fetch "/api/data" (obj :signal signal))))
```

---

## ID-27: Web Streams — `ReadableStream`, `WritableStream`, `TransformStream`

**Strength**: SHOULD

**Summary**: Deno uses the Web Streams API for streaming I/O.

```lykn
;; Good — consuming a stream with for-await-of
(bind response (await (fetch "/api/stream")))
(for-await-of chunk response:body
  (process chunk))
```

---

## ID-28: `.pipeThrough` and `.pipeTo` for Stream Composition

**Strength**: SHOULD

**Summary**: Chain transforms with `:pipeThrough` and direct output
with `:pipeTo`.

```lykn
;; Good — pipeline: read → decompress → decode → process
(bind response (await (fetch "/api/compressed-data")))
(bind text-stream (-> response:body
  (:pipeThrough (new DecompressionStream "gzip"))
  (:pipeThrough (new TextDecoderStream))))
(for-await-of text text-stream
  (process text))
```

---

## ID-29: Backpressure — Streams Handle It Automatically

**Strength**: CONSIDER

**Summary**: The Web Streams API manages backpressure internally. The
consumer's read speed controls the producer's output rate.

```lykn
;; Backpressure is automatic with pipeTo
(bind response (await (fetch "/api/large-file")))
(bind file (await (Deno:open "output.bin" (obj :write true :create true))))
(await (response:body:pipeTo file:writable))
```

---

## ID-30: Web Workers for CPU-Intensive Work

**Strength**: CONSIDER

**Summary**: Offload CPU-bound computation to Workers. Communicate via
`postMessage` — no shared state.

```lykn
;; main.lykn (compiled to main.js)
(bind worker (new Worker
  (-> (new URL "./worker.js" import:meta:url) :href)
  (obj :type "module" :deno (obj :permissions "inherit"))))

(worker:postMessage (obj :data large-data-set))
(worker:addEventListener "message" (fn (:any event)
  (console:log "Result:" event:data)))
```

---

## ID-31: `setTimeout` / `setInterval` Are Macrotasks

**Strength**: SHOULD

**Summary**: Timer callbacks are macrotasks — they run after all pending
microtasks.

```lykn
;; Good — promisified delay for use with await
(func delay :args (:number ms) :returns :any :body
  (new Promise (fn (:function resolve) (setTimeout resolve ms))))
(await (delay 1000))
```

---

## ID-32: Debouncing and Throttling with `cell`

**Strength**: SHOULD

**Summary**: Use `cell` for the mutable timer state that debounce and
throttle patterns require.

```lykn
;; Good — debounce: fire after events stop for wait ms
(func debounce :args (:function f :number wait) :returns :function :body
  (bind timer (cell null))
  (fn (:any args)
    (clearTimeout (express timer))
    (reset! timer (setTimeout (fn () (f args)) wait))))

;; Good — throttle: fire at most once per interval ms
(func throttle :args (:function f :number interval) :returns :function :body
  (bind last-event (cell null))
  (bind timer-id (cell null))
  (fn (:any args)
    (reset! last-event args)
    (if (= (express timer-id) null)
      (reset! timer-id (setTimeout (fn ()
        (f (express last-event))
        (reset! timer-id null)) interval)))))
```

**Rationale**: Debounce/throttle require mutable state (the timer ID).
In lykn, `cell` makes this explicit — the mutation points are visible
via `express`/`reset!`.

---

### Promise Combinator Decision Table

| Question | Use |
|----------|-----|
| Need ALL results, fail if any fail | `Promise:all` |
| Need ALL outcomes, partial failure OK | `Promise:allSettled` |
| Need FIRST success, ignore failures | `Promise:any` |
| Need FIRST settlement (timeout) | `Promise:race` |

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | Single-threaded, run-to-completion | MUST | No preemptive interruption |
| 02 | Microtasks before macrotasks | SHOULD | Promise callbacks drain first |
| 03 | Never block the event loop | MUST | Async I/O; Workers for CPU |
| 04 | `queueMicrotask` | CONSIDER | Microtask priority scheduling |
| 05 | Promises: sync start, async settle | MUST | Executor runs now; `:then` later |
| 06 | Resolving vs fulfilling | SHOULD | `resolve(promise)` locks-in |
| 07 | `Promise:withResolvers` | SHOULD | External settlement (ES2024) |
| 08 | `Promise:try` | CONSIDER | Uniform error handling (ES2025) |
| 09 | Promisification | SHOULD | Wrap callback APIs |
| 10 | Top-level `await` | SHOULD | No wrapper in ESM |
| 11 | Async start sync | MUST | Pre-await code runs eagerly |
| 12 | Async infectiousness | SHOULD | Callers must handle Promises |
| 13 | `Promise:all` for parallel | MUST | Fork-join; short-circuits |
| 14 | `Promise:allSettled` | SHOULD | Partial success → see 03 ID-16 |
| 15 | `Promise:any` | CONSIDER | First success → see 03 ID-17 |
| 16 | `Promise:race` | SHOULD | First settlement |
| 17 | Concurrency limiting | SHOULD | N-at-a-time worker pool |
| 18 | Sequential `for-of` + `await` | SHOULD | Order-dependent operations |
| 19 | `for-await-of` | SHOULD | Async iterable consumption |
| 20 | Async generators | SHOULD | `await` + `yield` |
| 21 | `Promise:all(items:map(async fn))` | MUST | `:map` returns `Promise[]` |
| 22 | `Array:fromAsync` | CONSIDER | Async iterable → array (ES2024) |
| 23 | `AbortController` / `AbortSignal` | SHOULD | Web Platform cancellation |
| 24 | Pass `signal` to `fetch` | MUST | Every production fetch |
| 25 | Check `signal:aborted` in loops | SHOULD | Cooperative cancellation |
| 26 | `AbortSignal:timeout` | SHOULD | Auto-abort after deadline |
| 27 | Web Streams API | SHOULD | Streaming I/O in Deno |
| 28 | `:pipeThrough` / `:pipeTo` | SHOULD | Composable stream pipelines |
| 29 | Backpressure is automatic | CONSIDER | Pull model prevents memory growth |
| 30 | Web Workers for CPU work | CONSIDER | Isolated thread, message passing |
| 31 | setTimeout/setInterval = macrotasks | SHOULD | Delay is a minimum |
| 32 | Debounce/throttle with `cell` | SHOULD | `cell` for mutable timer state |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for `for-of` (ID-25),
  `??` (ID-03), `some->` (ID-04)
- **Error Handling**: See `03-error-handling.md` for async error
  patterns (ID-13-22, ID-27-28)
- **Values & References**: See `04-values-references.md` for `cell`
  mutation model (ID-20)
- **Functions & Closures**: See `06-functions-closures.md` for closures
  (ID-06-10), generators (ID-05)
- **Performance**: See `08-performance.md` for lazy evaluation
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  `async`, `await`, `for-of`, `cell`
