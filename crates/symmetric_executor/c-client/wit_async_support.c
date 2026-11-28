#include "wit_async_support.h"
#include "executor_import.h"
#include "module.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ===========================================================================
// Thin wrappers of the generated bindings, using plain pointers
// ===========================================================================

#define EXE(name) symmetric_runtime_symmetric_executor_##name
#define STR(name) symmetric_runtime_symmetric_stream_##name

typedef uint8_t *handle_t;

static handle_t ev_dup(handle_t sub) {
  return EXE(method_event_subscription_dup)((EXE(borrow_event_subscription_t)){sub}).__handle;
}
static bool ev_ready(handle_t sub) {
  return EXE(method_event_subscription_ready)((EXE(borrow_event_subscription_t)){sub});
}
static void ev_reset(handle_t sub) {
  EXE(method_event_subscription_reset)((EXE(borrow_event_subscription_t)){sub});
}
static void ev_drop(handle_t sub) {
  EXE(event_subscription_drop_own)((EXE(own_event_subscription_t)){sub});
}
static handle_t gen_new(void) { return EXE(constructor_event_generator)().__handle; }
static handle_t gen_subscribe(handle_t gen) {
  return EXE(method_event_generator_subscribe)((EXE(borrow_event_generator_t)){gen}).__handle;
}
static void gen_activate(handle_t gen) {
  EXE(method_event_generator_activate)((EXE(borrow_event_generator_t)){gen});
}
static void gen_drop(handle_t gen) {
  EXE(event_generator_drop_own)((EXE(own_event_generator_t)){gen});
}
// callbacks return 1 (ready: don't call again) or 0 (call again on next event)
typedef uint32_t (*callback_t)(void *);
static void ex_register(handle_t trigger, callback_t callback, void *data) {
  EXE(own_callback_registration_t)
  registration =
      EXE(register)((EXE(own_event_subscription_t)){trigger},
                    (EXE(own_callback_function_t)){(uint8_t *)callback},
                    (EXE(own_callback_data_t)){(uint8_t *)data});
  EXE(callback_registration_drop_own)(registration);
}
static void ex_block_on(handle_t trigger) {
  EXE(block_on)((EXE(own_event_subscription_t)){trigger});
}

static handle_t buf_new(void *addr, uint64_t capacity, uint64_t elem_size) {
  return STR(constructor_buffer)((STR(own_address_t)){(uint8_t *)addr}, capacity, elem_size).__handle;
}
static void *buf_address(handle_t buf) {
  return STR(method_buffer_get_address)((STR(borrow_buffer_t)){buf}).__handle;
}
static uint64_t buf_size(handle_t buf) {
  return STR(method_buffer_get_size)((STR(borrow_buffer_t)){buf});
}
static uint64_t buf_capacity(handle_t buf) {
  return STR(method_buffer_capacity)((STR(borrow_buffer_t)){buf});
}
static void buf_set_size(handle_t buf, uint64_t size) {
  STR(method_buffer_set_size)((STR(borrow_buffer_t)){buf}, size);
}
static void buf_drop(handle_t buf) { STR(buffer_drop_own)((STR(own_buffer_t)){buf}); }

static handle_t stream_new(void) { return STR(constructor_stream_obj)().__handle; }
static handle_t stream_clone(handle_t stream, bool writing) {
  return STR(method_stream_obj_clone)((STR(borrow_stream_obj_t)){stream}, writing).__handle;
}
static void stream_drop(handle_t stream) {
  STR(stream_obj_drop_own)((STR(own_stream_obj_t)){stream});
}
static bool stream_is_read_closed(handle_t stream) {
  return STR(method_stream_obj_is_read_closed)((STR(borrow_stream_obj_t)){stream});
}
// returns whether the buffer was accepted, otherwise it is handed back
static bool stream_start_reading(handle_t stream, handle_t buf, handle_t *rejected) {
  STR(own_buffer_t) err = {NULL};
  bool ok = STR(method_stream_obj_start_reading)((STR(borrow_stream_obj_t)){stream},
                                                 (STR(own_buffer_t)){buf}, &err);
  *rejected = ok ? NULL : err.__handle;
  return ok;
}
// returns the buffer written by the other side, NULL if there is none (yet)
// or ever (`*eof`)
static handle_t stream_read_result(handle_t stream, bool *eof) {
  STR(own_buffer_t) buf = {NULL};
  STR(stream_state_t) state = 0;
  *eof = false;
  if (STR(method_stream_obj_read_result)((STR(borrow_stream_obj_t)){stream}, &buf, &state)) {
    return buf.__handle;
  }
  *eof = state == SYMMETRIC_RUNTIME_SYMMETRIC_STREAM_STREAM_STATE_EOF;
  return NULL;
}
// returns whether the buffer was accepted, otherwise it is handed back
static bool stream_offer_write(handle_t stream, handle_t buf, handle_t *rejected) {
  STR(own_buffer_t) err = {NULL};
  bool ok = STR(method_stream_obj_offer_write)((STR(borrow_stream_obj_t)){stream},
                                               (STR(own_buffer_t)){buf}, &err);
  *rejected = ok ? NULL : err.__handle;
  return ok;
}
// returns the completed write, NULL if it is still pending or the reader is
// gone (`*eof`)
static handle_t stream_write_result(handle_t stream, bool *eof) {
  STR(own_buffer_t) buf = {NULL};
  STR(stream_state_t) state = 0;
  *eof = false;
  if (STR(method_stream_obj_write_result)((STR(borrow_stream_obj_t)){stream}, &buf, &state)) {
    return buf.__handle;
  }
  *eof = state == SYMMETRIC_RUNTIME_SYMMETRIC_STREAM_STREAM_STATE_EOF;
  return NULL;
}
// the buffer of the operation (with the number of elements transferred so
// far) or NULL if nothing was in flight
static handle_t stream_cancel_read(handle_t stream) {
  STR(own_buffer_t) buf = {NULL};
  return STR(method_stream_obj_cancel_read)((STR(borrow_stream_obj_t)){stream}, &buf)
             ? buf.__handle
             : NULL;
}
static handle_t stream_cancel_write(handle_t stream) {
  STR(own_buffer_t) buf = {NULL};
  return STR(method_stream_obj_cancel_write)((STR(borrow_stream_obj_t)){stream}, &buf)
             ? buf.__handle
             : NULL;
}
static handle_t stream_read_ready_subscribe(handle_t stream) {
  return STR(method_stream_obj_read_ready_subscribe)((STR(borrow_stream_obj_t)){stream}).__handle;
}
static handle_t stream_write_ready_subscribe(handle_t stream) {
  return STR(method_stream_obj_write_ready_subscribe)((STR(borrow_stream_obj_t)){stream}).__handle;
}

// ===========================================================================
// Constants of the canonical ABI
// ===========================================================================

enum { CALLBACK_EXIT = 0, CALLBACK_YIELD = 1, CALLBACK_WAIT = 2 };
enum {
  EVENT_NONE = 0,
  EVENT_SUBTASK = 1,
  EVENT_STREAM_READ = 2,
  EVENT_STREAM_WRITE = 3,
  EVENT_FUTURE_READ = 4,
  EVENT_FUTURE_WRITE = 5,
};
enum { SUBTASK_STARTED = 1, SUBTASK_RETURNED = 2 };
enum { WAITABLE_COMPLETED = 0, WAITABLE_DROPPED = 1, WAITABLE_CANCELLED = 2 };
#define WAITABLE_BLOCKED ((uint32_t)-1)
#define WAITABLE_STATUS(state, count) ((uint32_t)(state) | ((uint32_t)(count) << 4))

// ===========================================================================
// State
// ===========================================================================

struct wit_task {
  uint8_t *retptr;      // where `_return` writes to
  handle_t return_gen;  // activated by `_return`
  bool returned;
  void *context[2];
  struct wit_task *caller;  // the task which was current when this one was entered
  // while it continues in the background
  uint32_t (*callback)(uint32_t, uintptr_t, uint32_t);
  handle_t trigger_gen;  // activated to run the callback
  uint32_t wait_set;     // set waited for, or 0
  struct wit_task *next, *prev;
};

struct wit_subtask {
  handle_t event;  // becomes ready when the call has returned
  bool returned;
  bool delivered;  // the event has been reported to the user
  bool used;
};

struct wit_waitable_set {
  uintptr_t *members;
  size_t len, capacity;
  bool used;
};

// A pending read or write of a stream or future. The memory of the operation
// is lent to the stream until the operation is reported (or cancelled).
struct wit_stream_op {
  uintptr_t handle;
  bool is_write;
  bool is_future;
  handle_t event;  // read-ready or write-ready subscription of the stream
  size_t elem_size;
  // reads: the caller's buffer
  void *buf;
  size_t amt;
  // writes: `count` elements offered at `mem`, which is the caller's buffer
  // or (when elements need to be duplicated for the reader) `dup`
  const void *mem;
  uint8_t *dup;
  size_t count;
  void (*disown)(void *, size_t);
};

// Not static: the dynamic linker makes all shared objects containing this
// file use the same one.
struct wit_async_state {
  struct wit_task *current;
  struct wit_task *tasks;  // list of tasks continuing in the background
  void *root_context[2];
  handle_t wake_gen;  // activated to leave a blocking wait
  struct wit_subtask *subtasks;
  size_t subtasks_len;
  struct wit_waitable_set *sets;
  size_t sets_len;
  struct wit_stream_op **ops;
  size_t ops_len;
} wit_async_state;

#define S wit_async_state

void wit_unsupported(const char *what) {
  fprintf(stderr, "%s is not supported by the symmetric ABI\n", what);
  abort();
}

// no other task is run: callbacks of the executor can't call into it
void wit_thread_yield(void) {}

// ===========================================================================
// Waiting for a set of waitables
// ===========================================================================

static uint32_t subtask_poll(uint32_t id, uint32_t *code);
static uint32_t op_poll(uintptr_t handle, uint32_t *code);

// the event of the waitable (if any) and the code to report, unless the event is
// reported already
static uint32_t waitable_poll(uintptr_t waitable, uint32_t *code) {
  if (waitable > 0 && waitable < S.subtasks_len + 1 && S.subtasks[waitable - 1].used) {
    return subtask_poll((uint32_t)waitable, code);
  }
  return op_poll(waitable, code);
}

static struct wit_stream_op *op_find(uintptr_t handle);

// A fresh subscription for each pending event of the set, which doesn't miss
// events happening from now on
static size_t set_events(uint32_t set, handle_t *events, size_t max) {
  size_t n = 0;
  if (set == 0 || set > S.sets_len || !S.sets[set - 1].used) {
    return 0;
  }
  struct wit_waitable_set *s = &S.sets[set - 1];
  for (size_t i = 0; i < s->len && n < max; i++) {
    uintptr_t w = s->members[i];
    handle_t event = NULL;
    if (w > 0 && w < S.subtasks_len + 1 && S.subtasks[w - 1].used) {
      struct wit_subtask *t = &S.subtasks[w - 1];
      if (!t->returned) {
        event = ev_dup(t->event);
      }
    } else {
      struct wit_stream_op *op = op_find(w);
      if (op) {
        // operations are polled, so they only need to be woken up by new events
        ev_reset(op->event);
        event = ev_dup(op->event);
      }
    }
    if (event) {
      events[n++] = event;
    }
  }
  return n;
}

static uint32_t set_poll(uint32_t set, uintptr_t *waitable, uint32_t *code) {
  *waitable = 0;
  *code = 0;
  if (set == 0 || set > S.sets_len || !S.sets[set - 1].used) {
    return EVENT_NONE;
  }
  struct wit_waitable_set *s = &S.sets[set - 1];
  for (size_t i = 0; i < s->len; i++) {
    uint32_t event = waitable_poll(s->members[i], code);
    if (event != EVENT_NONE) {
      *waitable = s->members[i];
      return event;
    }
  }
  *code = 0;
  return EVENT_NONE;
}

static uint32_t wake_up(void *data) {
  (void)data;
  gen_activate(S.wake_gen);
  return 1;
}

uint32_t wit_waitable_set_wait(uint32_t set, uintptr_t *waitable, uint32_t *code) {
  for (;;) {
    uint32_t event = set_poll(set, waitable, code);
    if (event != EVENT_NONE) {
      return event;
    }
    if (!S.wake_gen) {
      // never dropped: callbacks registered by a previous wait can still run
      S.wake_gen = gen_new();
    }
    handle_t wake = gen_subscribe(S.wake_gen);
    ev_reset(wake);
    handle_t events[64];
    size_t n = set_events(set, events, 64);
    if (n == 0) {
      // nothing which could ever become ready
      ev_drop(wake);
      return EVENT_NONE;
    }
    // something might have happened after the poll and before the events were
    // reset (in another thread)
    event = set_poll(set, waitable, code);
    if (event != EVENT_NONE) {
      for (size_t i = 0; i < n; i++) {
        ev_drop(events[i]);
      }
      ev_drop(wake);
      return event;
    }
    for (size_t i = 0; i < n; i++) {
      ex_register(events[i], wake_up, &S);  // the data must not be NULL
    }
    ex_block_on(wake);
  }
}

uint32_t wit_waitable_set_poll(uint32_t set, uintptr_t *waitable, uint32_t *code) {
  return set_poll(set, waitable, code);
}

uint32_t wit_waitable_set_new(void) {
  for (size_t i = 0; i < S.sets_len; i++) {
    if (!S.sets[i].used) {
      memset(&S.sets[i], 0, sizeof(S.sets[i]));
      S.sets[i].used = true;
      return (uint32_t)(i + 1);
    }
  }
  S.sets = realloc(S.sets, (S.sets_len + 1) * sizeof(struct wit_waitable_set));
  if (!S.sets) abort();
  memset(&S.sets[S.sets_len], 0, sizeof(struct wit_waitable_set));
  S.sets[S.sets_len].used = true;
  return (uint32_t)++S.sets_len;
}

static void set_remove(struct wit_waitable_set *s, uintptr_t waitable) {
  for (size_t i = 0; i < s->len; i++) {
    if (s->members[i] == waitable) {
      s->members[i] = s->members[--s->len];
      return;
    }
  }
}

void wit_waitable_join(uintptr_t waitable, uint32_t set) {
  // a waitable is a member of at most one set
  for (size_t i = 0; i < S.sets_len; i++) {
    if (S.sets[i].used) {
      set_remove(&S.sets[i], waitable);
    }
  }
  if (set == 0 || set > S.sets_len || !S.sets[set - 1].used) {
    return;
  }
  struct wit_waitable_set *s = &S.sets[set - 1];
  if (s->len == s->capacity) {
    s->capacity = s->capacity ? s->capacity * 2 : 4;
    s->members = realloc(s->members, s->capacity * sizeof(uintptr_t));
    if (!s->members) abort();
  }
  s->members[s->len++] = waitable;
}

void wit_waitable_set_drop(uint32_t set) {
  if (set == 0 || set > S.sets_len || !S.sets[set - 1].used) {
    return;
  }
  free(S.sets[set - 1].members);
  memset(&S.sets[set - 1], 0, sizeof(struct wit_waitable_set));
}

// ===========================================================================
// Subtasks
// ===========================================================================

uint32_t wit_subtask_status(uint8_t *event) {
  if (!event) {
    return SUBTASK_RETURNED;
  }
  size_t i;
  for (i = 0; i < S.subtasks_len && S.subtasks[i].used; i++) {
  }
  if (i == S.subtasks_len) {
    S.subtasks = realloc(S.subtasks, (S.subtasks_len + 1) * sizeof(struct wit_subtask));
    if (!S.subtasks) abort();
    S.subtasks_len++;
  }
  memset(&S.subtasks[i], 0, sizeof(struct wit_subtask));
  S.subtasks[i].used = true;
  S.subtasks[i].event = event;
  uint32_t id = (uint32_t)(i + 1);
  return SUBTASK_STARTED | (id << 4);
}

static uint32_t subtask_poll(uint32_t id, uint32_t *code) {
  struct wit_subtask *t = &S.subtasks[id - 1];
  if (!t->returned && ev_ready(t->event)) {
    t->returned = true;
  }
  if (t->returned && !t->delivered) {
    t->delivered = true;
    *code = SUBTASK_RETURNED;
    return EVENT_SUBTASK;
  }
  return EVENT_NONE;
}

uint32_t wit_subtask_cancel(uint32_t id) {
  if (id > 0 && id <= S.subtasks_len && S.subtasks[id - 1].used) {
    struct wit_subtask *t = &S.subtasks[id - 1];
    if (t->returned || ev_ready(t->event)) {
      t->returned = true;
      t->delivered = true;
      return SUBTASK_RETURNED;
    }
  }
  // the callee can't be asked to stop
  wit_unsupported("subtask.cancel of a running subtask");
  return 0;
}

void wit_subtask_drop(uint32_t id) {
  if (id > 0 && id <= S.subtasks_len && S.subtasks[id - 1].used) {
    wit_waitable_join(id, 0);
    ev_drop(S.subtasks[id - 1].event);
    memset(&S.subtasks[id - 1], 0, sizeof(struct wit_subtask));
  }
}

// ===========================================================================
// Tasks
// ===========================================================================

static struct wit_task *task_live(void *data) {
  for (struct wit_task *t = S.tasks; t; t = t->next) {
    if (t == data) return t;
  }
  return NULL;
}

// `trigger_gen` is leaked on purpose, pending wake up callbacks can still
// refer to it (they ignore tasks which are gone).
static void task_free(struct wit_task *t) {
  gen_drop(t->return_gen);
  free(t);
}

struct wit_task *wit_task_enter(uint8_t *retptr) {
  struct wit_task *t = calloc(1, sizeof(struct wit_task));
  if (!t) abort();
  t->retptr = retptr;
  t->return_gen = gen_new();
  t->caller = S.current;
  S.current = t;
  return t;
}

void wit_task_return(void) {
  struct wit_task *t = S.current;
  if (!t) {
    fprintf(stderr, "`_return` called outside of an async export\n");
    abort();
  }
  if (!t->returned) {
    t->returned = true;
    gen_activate(t->return_gen);
  }
}

uint8_t *wit_task_retptr(void) {
  if (!S.current) {
    fprintf(stderr, "`_return` called outside of an async export\n");
    abort();
  }
  return S.current->retptr;
}

void *wit_task_context_get(uint32_t slot) {
  return (S.current ? S.current->context : S.root_context)[slot];
}

void wit_task_context_set(uint32_t slot, void *value) {
  (S.current ? S.current->context : S.root_context)[slot] = value;
}

// The wake up callbacks of the events a task waits for
static uint32_t task_wake_up(void *data) {
  struct wit_task *t = task_live(data);
  if (t) {
    gen_activate(t->trigger_gen);
  }
  return 1;
}

static void task_wait_for_set(struct wit_task *t) {
  handle_t events[64];
  size_t n = set_events(t->wait_set, events, 64);
  for (size_t i = 0; i < n; i++) {
    ex_register(events[i], task_wake_up, t);
  }
}

// invoked by the executor
static uint32_t task_run(void *data) {
  struct wit_task *t = data;
  uint32_t event = EVENT_NONE, code = 0;
  uintptr_t waitable = 0;
  if (t->wait_set) {
    event = set_poll(t->wait_set, &waitable, &code);
    if (event == EVENT_NONE) {
      // woken up in vain
      task_wait_for_set(t);
      return 0;
    }
    t->wait_set = 0;
  }
  t->caller = S.current;
  S.current = t;
  uint32_t next = t->callback(event, waitable, code);
  S.current = t->caller;
  switch (next & 0xf) {
  case CALLBACK_YIELD:
    gen_activate(t->trigger_gen);
    return 0;
  case CALLBACK_WAIT:
    t->wait_set = next >> 4;
    // check what is there already first
    gen_activate(t->trigger_gen);
    return 0;
  default:
    break;
  }
  // exit
  if (!t->returned) {
    // `task.return` is mandatory, don't leave the caller waiting
    t->returned = true;
    gen_activate(t->return_gen);
  }
  if (t->next) t->next->prev = t->prev;
  if (t->prev) t->prev->next = t->next;
  else S.tasks = t->next;
  task_free(t);
  return 1;
}

uint8_t *wit_task_exit(struct wit_task *t, uint32_t code,
                       uint32_t (*callback)(uint32_t, uintptr_t, uint32_t)) {
  S.current = t->caller;
  if ((code & 0xf) == CALLBACK_EXIT) {
    // done, without having to wait for anything
    if (!t->returned) {
      t->returned = true;
      gen_activate(t->return_gen);
    }
    task_free(t);
    return NULL;
  }

  // If the result isn't available yet the caller gets an event to wait for.
  uint8_t *result_event = NULL;
  if (!t->returned) {
    result_event = gen_subscribe(t->return_gen);
    ev_reset(result_event);
  }

  // The rest of the task is run by the executor, but not before the caller is
  // back in control, so the trigger is activated last.
  t->callback = callback;
  t->wait_set = (code & 0xf) == CALLBACK_WAIT ? code >> 4 : 0;
  t->trigger_gen = gen_new();
  t->prev = NULL;
  t->next = S.tasks;
  if (S.tasks) S.tasks->prev = t;
  S.tasks = t;
  handle_t trigger = gen_subscribe(t->trigger_gen);
  ev_reset(trigger);
  ex_register(trigger, task_run, t);
  if (t->wait_set) {
    task_wait_for_set(t);
  }
  gen_activate(t->trigger_gen);
  return result_event;
}

// ===========================================================================
// Futures and streams
// ===========================================================================

// the buffer of elements without size, used for streams without payload
static uint8_t no_payload;

static struct wit_stream_op *op_find(uintptr_t handle) {
  for (size_t i = 0; i < S.ops_len; i++) {
    if (S.ops[i]->handle == handle) return S.ops[i];
  }
  return NULL;
}

static struct wit_stream_op *op_new(uintptr_t handle, bool is_write, bool is_future,
                                    size_t elem_size, handle_t event) {
  struct wit_stream_op *op = calloc(1, sizeof(struct wit_stream_op));
  if (!op) abort();
  op->handle = handle;
  op->is_write = is_write;
  op->is_future = is_future;
  op->elem_size = elem_size;
  // events which happened already are of no interest
  ev_reset(event);
  op->event = event;
  S.ops = realloc(S.ops, (S.ops_len + 1) * sizeof(struct wit_stream_op *));
  if (!S.ops) abort();
  S.ops[S.ops_len++] = op;
  return op;
}

static void op_free(struct wit_stream_op *op) {
  for (size_t i = 0; i < S.ops_len; i++) {
    if (S.ops[i] == op) {
      S.ops[i] = S.ops[--S.ops_len];
      break;
    }
  }
  ev_drop(op->event);
  free(op);
}

static uint32_t op_status(bool is_future, uint32_t code, size_t n) {
  return WAITABLE_STATUS(code, is_future ? 0 : n);
}

// The number of elements of a completed operation, the buffer is consumed
static size_t buf_take_count(handle_t buf) {
  size_t n = (size_t)buf_size(buf);
  buf_drop(buf);
  return n;
}

// The offered memory of the write, `n` elements of which have been taken by
// the reader (they belong to it now) and the others (which were duplicated for
// the offer) are given up. The caller's own elements are not touched.
static void write_release(struct wit_stream_op *op, size_t n) {
  if (op->dup) {
    if (op->disown && n < op->count) {
      op->disown(op->dup + n * op->elem_size, op->count - n);
    }
    free(op->dup);
    op->dup = NULL;
  }
}

// Offer the elements of the write, returns whether it is accepted. The
// runtime keeps the offset of a partly consumed write, the write stays
// pending until all is taken, the reader is gone or it is cancelled.
static bool write_offer(handle_t stream, const void *mem, size_t count, size_t elem_size) {
  handle_t buf = buf_new((void *)mem, count, elem_size);
  buf_set_size(buf, count);
  handle_t rejected;
  if (!stream_offer_write(stream, buf, &rejected)) {
    buf_drop(rejected);
    return false;
  }
  return true;
}

uintptr_t wit_stream_new(uintptr_t *writer) {
  handle_t stream = stream_new();
  handle_t reader = stream_clone(stream, false);
  *writer = (uintptr_t)stream;
  return (uintptr_t)reader;
}

// The status of a write which is over (everything taken or the reader gone),
// `n` is the number of elements taken in total. As in the canonical ABI the
// state is determined when the write is observed: a reader which is gone by
// then is reported (DROPPED with the count), even if everything was taken.
static uint32_t write_final_status(struct wit_stream_op *op, size_t n, bool eof) {
  bool closed = eof || stream_is_read_closed((handle_t)op->handle);
  return WAITABLE_STATUS(closed && (eof || !op->is_future) ? WAITABLE_DROPPED : WAITABLE_COMPLETED,
                         op->is_future ? 0 : n);
}

// Offer the caller's buffer to the stream. If a writer is waiting it completes
// right away (or the end of the stream is seen), as in the canonical ABI.
uint32_t wit_stream_read(uintptr_t reader, void *buf, size_t amt, size_t elem_size,
                         bool is_future) {
  if (amt == 0) {
    return op_status(is_future, WAITABLE_COMPLETED, 0);
  }
  handle_t stream = (handle_t)reader;
  void *mem = elem_size > 0 ? buf : &no_payload;
  handle_t rejected;
  if (!stream_start_reading(stream, buf_new(mem, amt, elem_size), &rejected)) {
    buf_drop(rejected);
    wit_unsupported("reading from a stream while the previous read is pending");
  }
  bool eof;
  handle_t result = stream_read_result(stream, &eof);
  if (result) {
    return op_status(is_future, WAITABLE_COMPLETED, buf_take_count(result));
  }
  if (eof) {
    return WAITABLE_STATUS(WAITABLE_DROPPED, 0);
  }
  struct wit_stream_op *op = op_new(reader, false, is_future, elem_size,
                                    stream_read_ready_subscribe(stream));
  op->buf = buf;
  op->amt = amt;
  return WAITABLE_BLOCKED;
}

// Offer the elements of the caller's buffer. If a reader is waiting and takes
// all of them this completes right away, otherwise the write is pending until
// all are consumed (the rest is offered to the next reads by the runtime).
uint32_t wit_stream_write(uintptr_t writer, const void *buf, size_t amt, size_t elem_size,
                          bool is_future, void (*own)(void *, size_t),
                          void (*disown)(void *, size_t)) {
  if (amt == 0) {
    return WAITABLE_STATUS(WAITABLE_COMPLETED, 0);
  }
  handle_t stream = (handle_t)writer;
  if (stream_is_read_closed(stream)) {
    return WAITABLE_STATUS(WAITABLE_DROPPED, 0);
  }
  struct wit_stream_op local = {0};
  local.handle = writer;
  local.is_write = true;
  local.is_future = is_future;
  local.elem_size = elem_size;
  local.count = amt;
  local.disown = disown;
  local.mem = elem_size > 0 ? buf : &no_payload;
  if (elem_size > 0 && own) {
    // reader and writer must not share memory, the copy is the reader's
    local.dup = malloc(amt * elem_size);
    if (!local.dup) abort();
    memcpy(local.dup, buf, amt * elem_size);
    own(local.dup, amt);
    local.mem = local.dup;
  }
  if (!write_offer(stream, local.mem, amt, elem_size)) {
    write_release(&local, 0);
    wit_unsupported("writing to a stream while the previous write is pending");
  }
  bool eof;
  handle_t result = stream_write_result(stream, &eof);
  if (result) {
    size_t n = buf_take_count(result);
    uint32_t status = write_final_status(&local, n, false);
    write_release(&local, n);
    return status;
  } else if (eof) {
    write_release(&local, 0);
    return WAITABLE_STATUS(WAITABLE_DROPPED, 0);
  }
  struct wit_stream_op *op = op_new(writer, true, is_future, elem_size,
                                    stream_write_ready_subscribe(stream));
  op->mem = local.mem;
  op->dup = local.dup;
  op->count = amt;
  op->disown = disown;
  return WAITABLE_BLOCKED;
}

// The event of a waiting read or write, if it is there
static uint32_t op_poll(uintptr_t handle, uint32_t *code) {
  struct wit_stream_op *op = op_find(handle);
  if (!op) {
    return EVENT_NONE;
  }
  handle_t stream = (handle_t)handle;
  bool eof;
  if (!op->is_write) {
    uint32_t event = op->is_future ? EVENT_FUTURE_READ : EVENT_STREAM_READ;
    handle_t result = stream_read_result(stream, &eof);
    if (result) {
      *code = op_status(op->is_future, WAITABLE_COMPLETED, buf_take_count(result));
    } else if (eof) {
      *code = WAITABLE_STATUS(WAITABLE_DROPPED, 0);
    } else {
      return EVENT_NONE;
    }
    op_free(op);
    return event;
  }
  uint32_t event = op->is_future ? EVENT_FUTURE_WRITE : EVENT_STREAM_WRITE;
  size_t n = 0;
  handle_t result = stream_write_result(stream, &eof);
  if (result) {
    n = buf_take_count(result);
    *code = write_final_status(op, n, false);
  } else if (eof) {
    *code = write_final_status(op, 0, true);
  } else {
    return EVENT_NONE;
  }
  write_release(op, n);
  op_free(op);
  return event;
}

uint32_t wit_stream_cancel_read(uintptr_t reader) {
  struct wit_stream_op *op = op_find(reader);
  if (!op) {
    return WAITABLE_STATUS(WAITABLE_CANCELLED, 0);
  }
  uint32_t code;
  if (op_poll(reader, &code) != EVENT_NONE) {
    return code;
  }
  // what was transferred meanwhile is reported (and not undone)
  handle_t buf = stream_cancel_read((handle_t)reader);
  size_t n = buf ? buf_take_count(buf) : 0;
  bool is_future = op->is_future;
  op_free(op);
  return n > 0 ? op_status(is_future, WAITABLE_COMPLETED, n)
               : WAITABLE_STATUS(WAITABLE_CANCELLED, 0);
}

uint32_t wit_stream_cancel_write(uintptr_t writer) {
  struct wit_stream_op *op = op_find(writer);
  if (!op) {
    return WAITABLE_STATUS(WAITABLE_COMPLETED, 0);
  }
  uint32_t code;
  if (op_poll(writer, &code) != EVENT_NONE) {
    return code;
  }
  handle_t buf = stream_cancel_write((handle_t)writer);
  size_t n = buf ? buf_take_count(buf) : 0;
  bool is_future = op->is_future;
  bool all = n == op->count;
  write_release(op, n);
  op_free(op);
  if (all) {
    return op_status(is_future, WAITABLE_COMPLETED, n);
  }
  return WAITABLE_STATUS(WAITABLE_CANCELLED, is_future ? 0 : n);
}

void wit_stream_drop_readable(uintptr_t reader) {
  if (op_find(reader)) {
    // the memory of the read must not be referenced anymore
    wit_stream_cancel_read(reader);
  }
  wit_waitable_join(reader, 0);
  stream_drop((handle_t)reader);
}

void wit_stream_drop_writable(uintptr_t writer) {
  if (op_find(writer)) {
    wit_stream_cancel_write(writer);
  }
  wit_waitable_join(writer, 0);
  stream_drop((handle_t)writer);
}
