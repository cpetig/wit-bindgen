#pragma once
#ifndef WIT_BINDGEN_WIT_H_DEFINED
#define WIT_BINDGEN_WIT_H_DEFINED

#ifndef WIT_ARA_SPAN // AUTOSAR (C++17) uses ara::core::Result
#include <expected> // needs to precede optional
#endif
#include <assert.h>
#include <map>
#include <optional>
#include <stddef.h> // size_t
#include <stdint.h>
#include <memory> // unique_ptr
#include <stdint.h>
#include <string>
#include <string_view>
#include <string.h> // memcpy
#include <stdlib.h> // free
#include <new>
#ifdef WIT_ARA_SPAN
#include <ara/core/span.h>
#elif __cplusplus >= 202002L
#include <span>
#endif
#include <type_traits>
#include <utility> // pair
#include <vector>
#ifdef __cpp_impl_coroutine
#include <coroutine>
#endif

namespace wit {
#ifdef WIT_ARA_SPAN
template <class T> using span=::ara::core::Span<T>;
#elif __cplusplus >= 202002L
template <class T> using span=std::span<T>;
#else
/// Minimal replacement of std::span for C++17 (a view of contiguous elements),
/// the C++17 bindings (`--async-api future`) use it where C++20 uses std::span
template <class T> class span {
  T *data_;
  size_t size_;

public:
  using element_type = T;
  using value_type = std::remove_cv_t<T>;
  using size_type = size_t;
  using pointer = T *;
  using reference = T &;
  using iterator = T *;

  constexpr span() noexcept : data_(nullptr), size_(0) {}
  constexpr span(T *ptr, size_t count) noexcept : data_(ptr), size_(count) {}
  template <size_t N>
  constexpr span(T (&arr)[N]) noexcept : data_(arr), size_(N) {}
  template <class C, class = std::enable_if_t<std::is_convertible_v<
                         std::remove_pointer_t<decltype(std::declval<C &>().data())> (*)[],
                         T (*)[]>>>
  constexpr span(C &c) : data_(c.data()), size_(c.size()) {}
  template <class U, class = std::enable_if_t<std::is_convertible_v<U (*)[], T (*)[]>>>
  constexpr span(span<U> const &o) noexcept : data_(o.data()), size_(o.size()) {}

  constexpr T *data() const noexcept { return data_; }
  constexpr size_t size() const noexcept { return size_; }
  constexpr bool empty() const noexcept { return size_ == 0; }
  constexpr T *begin() const noexcept { return data_; }
  constexpr T *end() const noexcept { return data_ + size_; }
  constexpr T &operator[](size_t n) const { return data_[n]; }
  constexpr T &front() const { return data_[0]; }
  constexpr T &back() const { return data_[size_ - 1]; }
};
#endif
/// std::bit_cast (C++20) for all language versions
template <class To, class From> To bit_cast(From const &from) noexcept {
  static_assert(sizeof(To) == sizeof(From), "size mismatch");
  To to;
  memcpy(static_cast<void *>(&to), static_cast<void const *>(&from), sizeof(To));
  return to;
}
/// @brief Helper class to map between IDs and resources
/// @tparam R Type of the Resource
template <class R> class ResourceTable {
  static std::map<int32_t, R> resources;

public:
  static R *lookup_resource(int32_t id) {
    auto result = resources.find(id);
    return result == resources.end() ? nullptr : &result->second;
  }
  static int32_t store_resource(R &&value) {
    auto last = resources.rbegin();
    int32_t id = last == resources.rend() ? 0 : last->first + 1;
    resources.insert(std::pair<int32_t, R>(id, std::move(value)));
    return id;
  }
  static std::optional<R> remove_resource(int32_t id) {
    auto iter = resources.find(id);
    std::optional<R> result;
    if (iter != resources.end()) {
      result.emplace(std::move(iter->second));
      resources.erase(iter);
    }
    return std::move(result);
  }
};

/// @brief Replaces void in the error position of a result
struct Void {};

/// A string in linear memory, freed unconditionally using free
///
/// A normal C++ string makes no guarantees about where the characters
/// are stored and how this is freed.
class string {
  uint8_t const *data_;
  size_t length;
  // C++ is horrible!
  //constexpr uint8_t const *const empty_ptr = (uint8_t const *)1;
  static uint8_t const* empty_ptr() { return (uint8_t const *)1; }

public:
  // this constructor is helpful for creating vector<string>
  string(string const &b) : string(string::from_view(b.get_view())) {}
  string(string &&b) noexcept : data_(b.data_), length(b.length) { b.data_ = nullptr; }
  string &operator=(string const &) = delete;
  string &operator=(string &&b) noexcept {
    if (data_ && data_!=empty_ptr()) {
      free(const_cast<uint8_t *>(data_));
    }
    data_ = b.data_;
    length = b.length;
    b.data_ = nullptr;
    return *this;
  }
  string(char const *d, size_t l) : data_((uint8_t const *)d), length(l) {}
  char const *data() const { return (char const *)data_; }
  size_t size() const { return length; }
  bool empty() const { return !length; }
  ~string() {
    if (data_ && data_!=empty_ptr()) {
      free(const_cast<uint8_t *>(data_));
    }
  }
  // leak the memory
  void leak() { data_ = nullptr; }
  // typically called by post
  static void drop_raw(void *ptr) { free(ptr); }
  std::string_view get_view() const {
    return std::string_view((const char *)data_, length);
  }
  std::string to_string() const {
    return std::string((const char *)data_, length);
  }
  static string from_view(std::string_view v) {
    if (!v.size()) return string((char const*)empty_ptr(), 0);
    char* addr = (char*)malloc(v.size());
    memcpy(addr, v.data(), v.size());
    return string(addr, v.size());
  }
  char* begin() {
    return (char*)data_;
  }
  char* end() {
    return (char*)data_ + length;
  }
  char const* begin() const {
    return (char const*)data_;
  }
  char const* end() const {
    return (char const*)data_ + length;
  }
};

/// A vector in linear memory, freed unconditionally using free
///
/// You can't detach the data memory from a vector, nor create one
/// in a portable way from a buffer and lenght without copying.
template <class T> class vector {
  T *data_;
  size_t length;

  static T* empty_ptr() { return (T*)alignof(T); }

public:
  vector(vector const &) = delete;
  vector(vector &&b) noexcept : data_(b.data_), length(b.length) { b.data_ = nullptr; }
  vector &operator=(vector const &) = delete;
  vector &operator=(vector &&b) noexcept {
    if (data_ && length>0) {
      free(data_);
    }
    data_ = b.data_;
    length = b.length;
    b.data_ = nullptr;
    return *this;
  }
  vector(T *d, size_t l) : data_(d), length(l) {}
  // Rust needs a nonzero pointer here (alignment is typical)
  vector() : data_(empty_ptr()), length() {}
  T const *data() const { return data_; }
  T *data() { return data_; }
  T &operator[](size_t n) { return data_[n]; }
  T const &operator[](size_t n) const { return data_[n]; }
  size_t size() const { return length; }
  bool empty() const { return !length; }
  ~vector() {
    if (data_ && length>0) {
      for (unsigned i=0;i<length;++i) { data_[i].~T(); }
      free((void*)data_);
    }
  }
  // WARNING: vector contains uninitialized elements
  static vector<T> allocate(size_t len) {
    if (!len) return vector<T>(empty_ptr(), 0);
    return vector<T>((T*)malloc(sizeof(T)*len), len);
  }
  void initialize(size_t n, T&& elem) {
    new ((void*)(data_+n)) T(std::move(elem));
  }
  // leak the memory
  T* leak() { T*result = data_; data_ = nullptr; return result; }
  // typically called by post
  static void drop_raw(void *ptr) { if (ptr!=empty_ptr()) free(ptr); }
  wit::span<T> get_view() const { return wit::span<T>(data_, length); }
  wit::span<const T> get_const_view() const { return wit::span<const T>(data_, length); }
  template <class U> static vector<T> from_view(wit::span<U> const& a) {
    auto result = vector<T>::allocate(a.size());
    for (uint32_t i=0;i<a.size();++i) {
      new ((void*)(result.data_+i)) T(a[i]);
    }
    return result;
  } 
};

/// @brief A map stored as a contiguous array of key-value pairs in linear
/// memory, freed unconditionally using free.
///
/// Mirrors the canonical ABI representation of `map<K, V>` (`list<tuple<K, V>>`)
/// to enable lift without per-entry tree allocation. The container has no
/// ordering or hashing guarantees and exposes only the subset of the
/// `std::unordered_map` API that's meaningful over a flat pair buffer plus
/// the bindings-construction primitives (`allocate`, `initialize`, `leak`,
/// `drop_raw`, `data`).
template <class K, class V> class unordered_map {
public:
  using entry_type = std::pair<K, V>;

private:
  entry_type *data_;
  size_t length;

  static entry_type* empty_ptr() { return (entry_type*)alignof(entry_type); }

public:
  unordered_map(unordered_map const &) = delete;
  unordered_map(unordered_map &&b) noexcept : data_(b.data_), length(b.length) {
    b.data_ = nullptr;
    b.length = 0;
  }
  unordered_map &operator=(unordered_map const &) = delete;
  unordered_map &operator=(unordered_map &&b) noexcept {
    if (data_ && length > 0) {
      for (unsigned i = 0; i < length; ++i) { data_[i].~entry_type(); }
      free(data_);
    }
    data_ = b.data_;
    length = b.length;
    b.data_ = nullptr;
    b.length = 0;
    return *this;
  }
  unordered_map(entry_type *d, size_t l) : data_(d), length(l) {}
  unordered_map() : data_(empty_ptr()), length() {}
  entry_type const *data() const { return data_; }
  entry_type *data() { return data_; }
  size_t size() const { return length; }
  bool empty() const { return !length; }
  ~unordered_map() {
    if (data_ && length > 0) {
      for (unsigned i = 0; i < length; ++i) { data_[i].~entry_type(); }
      free((void*)data_);
    }
  }
  // WARNING: unordered_map contains uninitialized entries; caller must
  // construct them via `initialize` before the map is observed or destroyed.
  static unordered_map<K, V> allocate(size_t len) {
    if (!len) return unordered_map<K, V>(empty_ptr(), 0);
    return unordered_map<K, V>(
        (entry_type*)malloc(sizeof(entry_type) * len), len);
  }
  void initialize(size_t n, entry_type&& entry) {
    new ((void*)(data_ + n)) entry_type(std::move(entry));
  }
  entry_type *leak() {
    entry_type *result = data_;
    data_ = nullptr;
    return result;
  }
  static void drop_raw(void *ptr) {
    if (ptr != empty_ptr()) free(ptr);
  }
  entry_type *begin() { return data_; }
  entry_type *end() { return data_ + length; }
  entry_type const *begin() const { return data_; }
  entry_type const *end() const { return data_ + length; }
};

/// @brief  A Resource defined within the guest (guest side)
///
/// It registers with the host and should remain in a static location.
/// Typically referenced by the Owned type
///
/// Note that deregistering will cause the host to call Dtor which
/// in turn frees the object.
template <class R> class ResourceExportBase {
public:
  struct Deregister {
    void operator()(R *ptr) const {
      // probably always true because of unique_ptr wrapping, TODO: check
#ifdef WIT_SYMMETRIC
      if (ptr->handle != nullptr)
#else
      if (ptr->handle >= 0)
#endif
      {
        // we can't deallocate because the host calls Dtor
        R::ResourceDrop(ptr->handle);
      }
    }
  };
  typedef std::unique_ptr<R, Deregister> Owned;

#ifdef WIT_SYMMETRIC
  typedef uint8_t *handle_t;
  static constexpr handle_t invalid = nullptr;
#else
  typedef int32_t handle_t;
  static const handle_t invalid = -1;
#endif

  handle_t handle;

  ResourceExportBase() : handle(R::ResourceNew((R *)this)) {}
  // because this function is called by the host via Dtor we must not deregister
  ~ResourceExportBase() {}
  ResourceExportBase(ResourceExportBase const &) = delete;
  ResourceExportBase(ResourceExportBase &&) = delete;
  ResourceExportBase &operator=(ResourceExportBase &&b) = delete;
  ResourceExportBase &operator=(ResourceExportBase const &) = delete;
  handle_t get_handle() const { return handle; }
  handle_t into_handle() {
    handle_t result = handle;
    handle = invalid;
    return result;
  }
};

/// @brief A Resource imported from the host (guest side)
///
/// Wraps the identifier and can be forwarded but not duplicated
class ResourceImportBase {
public:
#ifdef WIT_SYMMETRIC
  typedef uint8_t *handle_t;
  static constexpr handle_t invalid = nullptr;
#else
  typedef int32_t handle_t;
  static const handle_t invalid = -1;
#endif

protected:
  handle_t handle;

public:
  ResourceImportBase(handle_t h = invalid) : handle(h) {}
  ResourceImportBase(ResourceImportBase &&r) noexcept : handle(r.handle) {
    r.handle = invalid;
  }
  ResourceImportBase(ResourceImportBase const &) = delete;
  void set_handle(handle_t h) { handle = h; }
  handle_t get_handle() const { return handle; }
  handle_t into_handle() {
    handle_t h = handle;
    handle = invalid;
    return h;
  }
  ResourceImportBase &operator=(ResourceImportBase &&r) noexcept {
    assert(handle == invalid);
    handle = r.handle;
    r.handle = invalid;
    return *this;
  }
  ResourceImportBase &operator=(ResourceImportBase const &r) = delete;
};

enum SubtaskState : uint32_t {
  SUBTASK_STARTING = 0,
  SUBTASK_STARTED = 1,
  SUBTASK_RETURNED = 2,
  SUBTASK_STARTED_CANCELLED = 3,
  SUBTASK_RETURNED_CANCELLED = 4,
};

enum CallbackCode : uint32_t {
  CALLBACK_CODE_EXIT = 0,
  CALLBACK_CODE_YIELD = 1,
  CALLBACK_CODE_WAIT = 2,
};

enum EventCode : uint32_t {
  EVENT_NONE = 0,
  EVENT_SUBTASK = 1,
  EVENT_STREAM_READ = 2,
  EVENT_STREAM_WRITE = 3,
  EVENT_FUTURE_READ = 4,
  EVENT_FUTURE_WRITE = 5,
  EVENT_CANCEL = 6,
};

enum WaitableState : uint32_t {
  WAITABLE_COMPLETED = 0,
  WAITABLE_DROPPED = 1,
  WAITABLE_CANCELLED = 2,
};

inline constexpr uint32_t WAITABLE_STATUS_BLOCKED = 0xffffffffU;

// async support needs C++20 coroutines
#ifdef __cpp_impl_coroutine
namespace detail {

#ifdef __wasm__
__attribute__((__import_module__("$root"), __import_name__("[waitable-set-new]")))
extern "C" uint32_t wit_async_waitable_set_new();

__attribute__((__import_module__("$root"), __import_name__("[waitable-join]")))
extern "C" void wit_async_waitable_join(uint32_t waitable, uint32_t set);

__attribute__((__import_module__("$root"), __import_name__("[waitable-set-wait]")))
extern "C" uint32_t wit_async_waitable_set_wait(uint32_t set, uint32_t *payload);

__attribute__((__import_module__("$root"), __import_name__("[waitable-set-poll]")))
extern "C" uint32_t wit_async_waitable_set_poll(uint32_t set, uint32_t *payload);

__attribute__((__import_module__("$root"), __import_name__("[waitable-set-drop]")))
extern "C" void wit_async_waitable_set_drop(uint32_t set);

__attribute__((__import_module__("$root"), __import_name__("[subtask-drop]")))
extern "C" void wit_async_subtask_drop(uint32_t subtask);

__attribute__((__import_module__("$root"), __import_name__("[subtask-cancel]")))
extern "C" uint32_t wit_async_subtask_cancel(uint32_t subtask);

__attribute__((__import_module__("$root"), __import_name__("[thread-yield]")))
extern "C" uint32_t wit_async_thread_yield();

__attribute__((__import_module__("$root"), __import_name__("[backpressure-inc]")))
extern "C" void wit_async_backpressure_inc();

__attribute__((__import_module__("$root"), __import_name__("[backpressure-dec]")))
extern "C" void wit_async_backpressure_dec();

__attribute__((__import_module__("[export]$root"), __import_name__("[task-cancel]")))
extern "C" void wit_async_task_cancel();

#ifdef __wasm_libcall_thread_context__
inline void *&wit_async_tls_slot() {
  static _Thread_local void *slot = nullptr;
  return slot;
}
inline void *wit_async_context_get() { return wit_async_tls_slot(); }
inline void wit_async_context_set(void *val) { wit_async_tls_slot() = val; }
#else
__attribute__((__import_module__("$root"), __import_name__("[context-get-0]")))
extern "C" void *wit_async_context_get_0();

__attribute__((__import_module__("$root"), __import_name__("[context-set-0]")))
extern "C" void wit_async_context_set_0(void *val);

inline void *wit_async_context_get() { return wit_async_context_get_0(); }
inline void wit_async_context_set(void *val) { wit_async_context_set_0(val); }
#endif

#else
inline uint32_t wit_async_waitable_set_new() { return 1; }
inline void wit_async_waitable_join(uint32_t, uint32_t) {}
inline uint32_t wit_async_waitable_set_wait(uint32_t, uint32_t *) { return 0; }
inline uint32_t wit_async_waitable_set_poll(uint32_t, uint32_t *) { return 0; }
inline void wit_async_waitable_set_drop(uint32_t) {}
inline void wit_async_subtask_drop(uint32_t) {}
inline uint32_t wit_async_subtask_cancel(uint32_t) { return 0; }
inline uint32_t wit_async_thread_yield() { return 0; }
inline void wit_async_backpressure_inc() {}
inline void wit_async_backpressure_dec() {}
inline void wit_async_task_cancel() {}
inline void *&wit_async_tls_slot() {
  static void *slot = nullptr;
  return slot;
}
inline void *wit_async_context_get() { return wit_async_tls_slot(); }
inline void wit_async_context_set(void *val) { wit_async_tls_slot() = val; }
#endif

#ifdef WIT_SYMMETRIC
// Access to the symmetric executor (see crates/symmetric_executor/wit). The
// symbols are declared here directly, as the generated bindings of the
// executor include this header.
#define WIT_SYM_EXE(x) symmetricX3AruntimeX2Fsymmetric_executorX400X2E3X2E1X00##x
extern "C" uint8_t *WIT_SYM_EXE(X5BconstructorX5Devent_generator)();
extern "C" uint8_t *WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Esubscribe)(uint8_t *);
extern "C" void WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Eactivate)(uint8_t *);
extern "C" void WIT_SYM_EXE(X5Bresource_dropX5Devent_generator)(uint8_t *);
extern "C" void WIT_SYM_EXE(X5Bresource_dropX5Devent_subscription)(uint8_t *);
extern "C" void WIT_SYM_EXE(X5Bresource_dropX5Dcallback_registration)(uint8_t *);
extern "C" uint8_t *WIT_SYM_EXE(register)(uint8_t *, uint8_t *, uint8_t *);
extern "C" void WIT_SYM_EXE(block_on)(uint8_t *);

/// Register `callback(data)` to be called once `event` (consumed) triggers.
/// The callback can be called immediately if the event is active already.
inline void sym_register(uint8_t *event, uint8_t (*callback)(void *),
                         void *data) {
  uint8_t *reg = WIT_SYM_EXE(register)(event, reinterpret_cast<uint8_t *>(callback),
                                       reinterpret_cast<uint8_t *>(data));
  if (reg) {
    WIT_SYM_EXE(X5Bresource_dropX5Dcallback_registration)(reg);
  }
}

/// Wait for a subscription returned by an async call of the symmetric ABI
/// (null means that the call has completed already).
///
/// The callback of the executor refers to a heap allocated state, so it is
/// safe to destroy the coroutine while it still waits.
struct EventAwaiter {
  struct State {
    std::coroutine_handle<> handle = nullptr;
    bool registering = false;
    bool fired = false;
    bool dead = false;
  };

  uint8_t *event;
  State *state = nullptr;

  explicit EventAwaiter(void *e) : event(static_cast<uint8_t *>(e)) {}
  EventAwaiter(const EventAwaiter &) = delete;
  EventAwaiter &operator=(const EventAwaiter &) = delete;
  ~EventAwaiter() {
    if (state) {
      // abandoned while waiting, the callback cleans up
      state->dead = true;
    }
  }

  bool await_ready() const noexcept { return event == nullptr; }
  bool await_suspend(std::coroutine_handle<> h) noexcept {
    state = new State;
    state->handle = h;
    state->registering = true;
    sym_register(event, &EventAwaiter::on_event, state);
    state->registering = false;
    // the executor calls back immediately for events which are active
    if (state->fired) {
      delete state;
      state = nullptr;
      return false;
    }
    return true;
  }
  void await_resume() noexcept { state = nullptr; }

  static uint8_t on_event(void *data) {
    auto *s = static_cast<State *>(data);
    if (s->registering) {
      s->fired = true;
    } else {
      bool dead = s->dead;
      auto handle = s->handle;
      delete s;
      if (!dead) {
        handle.resume();
      }
    }
    return 1; // CallbackState::ready
  }
};

/// A coroutine which nobody waits for, it destroys itself when done.
struct Detached {
  struct promise_type {
    Detached get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {
      assert(false && "unhandled exception in wit::Task");
    }
  };
};

/// Per call state of an async export, see `start_task_symmetric`.
struct ExportTask;
#endif

struct WaitableEntry {
  uint32_t waitable = 0;
  std::coroutine_handle<> continuation = nullptr;
  uint32_t *out_code = nullptr;
  bool is_subtask = false;
  bool active = false;
  uint32_t (*cancel_fn)(uint32_t) = nullptr;
};

struct AsyncTaskContext {
  std::coroutine_handle<> root_handle = nullptr;
  bool owns_root = true;
  uint32_t waitable_set = 0;
  bool should_yield = false;
  bool cancelled = false;
  std::vector<WaitableEntry> entries;
  std::vector<std::coroutine_handle<>> ready_queue;
  std::vector<std::coroutine_handle<>> spawned_handles;

  AsyncTaskContext() = default;
  ~AsyncTaskContext() {
    destroy_coroutines();
    for (auto &entry : entries) {
      if (entry.active) {
        entry.active = false;
        wit_async_waitable_join(entry.waitable, 0);
        if (entry.cancel_fn) {
          entry.cancel_fn(entry.waitable);
        }
        if (entry.is_subtask) {
          wit_async_subtask_drop(entry.waitable);
        }
      }
    }
    if (waitable_set != 0) {
      wit_async_waitable_set_drop(waitable_set);
      waitable_set = 0;
    }
  }

  AsyncTaskContext(const AsyncTaskContext &) = delete;
  AsyncTaskContext &operator=(const AsyncTaskContext &) = delete;

  void destroy_coroutines() {
    for (auto h : spawned_handles) {
      if (h) {
        h.destroy();
      }
    }
    spawned_handles.clear();
    if (owns_root && root_handle) {
      auto h = root_handle;
      root_handle = nullptr;
      h.destroy();
    }
  }

  void cleanup_finished() {
    for (size_t i = 0; i < spawned_handles.size();) {
      auto h = spawned_handles[i];
      if (!h || h.done()) {
        if (h) {
          h.destroy();
        }
        spawned_handles.erase(spawned_handles.begin() + i);
      } else {
        ++i;
      }
    }
    if (owns_root && root_handle && root_handle.done()) {
      auto h = root_handle;
      root_handle = nullptr;
      h.destroy();
    }
  }

  bool has_active_waitables() const {
    for (const auto &entry : entries) {
      if (entry.active) {
        return true;
      }
    }
    return false;
  }

  bool has_pending() const {
    return !ready_queue.empty() || has_active_waitables();
  }

  bool is_done() const {
    if (cancelled) {
      return true;
    }
    bool root_done = !root_handle || root_handle.done();
    return root_done && spawned_handles.empty() && !has_pending();
  }

  void register_waitable(uint32_t waitable, std::coroutine_handle<> cont,
                         uint32_t *out_code, bool is_subtask,
                         uint32_t (*cancel_fn)(uint32_t) = nullptr) {
    if (waitable_set == 0) {
      waitable_set = wit_async_waitable_set_new();
    }
    wit_async_waitable_join(waitable, waitable_set);
    for (auto &entry : entries) {
      if (!entry.active) {
        entry = WaitableEntry{waitable, cont, out_code, is_subtask, true, cancel_fn};
        return;
      }
    }
    entries.push_back(
        WaitableEntry{waitable, cont, out_code, is_subtask, true, cancel_fn});
  }

  void cancel_ready(std::coroutine_handle<> cont) {
    for (size_t i = 0; i < ready_queue.size();) {
      if (ready_queue[i] == cont) {
        ready_queue.erase(ready_queue.begin() + i);
      } else {
        ++i;
      }
    }
  }

  void cancel_waitable(uint32_t waitable, std::coroutine_handle<> cont) {
    cancel_ready(cont);
    for (auto &entry : entries) {
      if (entry.active && entry.waitable == waitable &&
          entry.continuation == cont) {
        entry.active = false;
        wit_async_waitable_join(waitable, 0);
        uint32_t code = entry.cancel_fn ? entry.cancel_fn(waitable) : 0;
        if (entry.out_code) {
          *entry.out_code = code;
        }
        if (entry.is_subtask) {
          wit_async_subtask_drop(waitable);
        }
        return;
      }
    }
  }

  void deliver_event(uint32_t event0, uint32_t event1, uint32_t event2) {
    if (event0 == EVENT_NONE) {
      return;
    }
    if (event0 == EVENT_CANCEL) {
      cancelled = true;
      ready_queue.clear();
      destroy_coroutines();
      for (auto &entry : entries) {
        if (entry.active) {
          entry.active = false;
          wit_async_waitable_join(entry.waitable, 0);
          if (entry.cancel_fn) {
            entry.cancel_fn(entry.waitable);
          }
          if (entry.is_subtask) {
            wit_async_subtask_drop(entry.waitable);
          }
        }
      }
      wit_async_task_cancel();
      return;
    }
    for (auto &entry : entries) {
      if (entry.active && entry.waitable == event1) {
        if (entry.is_subtask) {
          if (entry.out_code) {
            *entry.out_code = event2;
          }
          uint32_t state = event2 & 0xfU;
          if (state != SUBTASK_RETURNED &&
              state != SUBTASK_STARTED_CANCELLED &&
              state != SUBTASK_RETURNED_CANCELLED) {
            return;
          }
        }
        entry.active = false;
        wit_async_waitable_join(event1, 0);
        if (entry.is_subtask) {
          wit_async_subtask_drop(event1);
        }
        if (entry.out_code) {
          *entry.out_code = event2;
        }
        if (entry.continuation) {
          ready_queue.push_back(entry.continuation);
        }
        return;
      }
    }
  }

  uint32_t step() {
    cleanup_finished();
    if (is_done()) {
      return CALLBACK_CODE_EXIT;
    }
    for (;;) {
      while (!ready_queue.empty() && !should_yield) {
        size_t batch = ready_queue.size();
        for (size_t i = 0; i < batch && !ready_queue.empty(); ++i) {
          auto h = ready_queue.front();
          ready_queue.erase(ready_queue.begin());
          h.resume();
          cleanup_finished();
          if (is_done()) {
            return CALLBACK_CODE_EXIT;
          }
        }
      }
      if (is_done()) {
        return CALLBACK_CODE_EXIT;
      }
      if (should_yield) {
        if (waitable_set != 0 && has_active_waitables()) {
          uint32_t payload[2] = {0, 0};
          uint32_t ev = wit_async_waitable_set_poll(waitable_set, payload);
          if (ev != EVENT_NONE) {
            should_yield = false;
            deliver_event(ev, payload[0], payload[1]);
            if (is_done()) {
              return CALLBACK_CODE_EXIT;
            }
            continue;
          }
        }
        should_yield = false;
        return CALLBACK_CODE_YIELD;
      }
      assert(waitable_set != 0);
      return CALLBACK_CODE_WAIT | (waitable_set << 4);
    }
  }
};

inline AsyncTaskContext *&current_context() {
  static AsyncTaskContext *ctx = nullptr;
  return ctx;
}

inline AsyncTaskContext *ensure_current_context() {
  auto *&ctx = current_context();
  if (!ctx) {
    ctx = new AsyncTaskContext();
  }
  return ctx;
}

struct SubtaskAwaiter {
  using CleanupList = std::vector<std::pair<void (*)(uint32_t), uint32_t>>;
  uint32_t packed_status;
  uint32_t final_status = 0;
  CleanupList cleanup;
  AsyncTaskContext *ctx = nullptr;
  std::coroutine_handle<> handle = nullptr;
  bool pending = false;

  explicit SubtaskAwaiter(uint32_t status, CleanupList c = {})
      : packed_status(status), final_status(status), cleanup(std::move(c)) {
    if ((packed_status & 0xfU) != SUBTASK_STARTING) {
      cleanup.clear();
    }
  }
  SubtaskAwaiter(const SubtaskAwaiter &) = delete;
  SubtaskAwaiter &operator=(const SubtaskAwaiter &) = delete;

  ~SubtaskAwaiter() {
    if (pending && ctx) {
      ctx->cancel_waitable(packed_status >> 4, handle);
      if ((final_status & 0xfU) == SUBTASK_STARTED_CANCELLED) {
        for (auto &item : cleanup) {
          item.first(item.second);
        }
      }
    }
  }

  bool await_ready() const noexcept {
    return (packed_status & 0xfU) == SUBTASK_RETURNED;
  }
  void await_suspend(std::coroutine_handle<> h) noexcept {
    ctx = ensure_current_context();
    handle = h;
    pending = true;
    uint32_t subtask = packed_status >> 4;
    ctx->register_waitable(
        subtask, h, &final_status, /*is_subtask=*/true, wit_async_subtask_cancel);
  }
  uint32_t await_resume() noexcept {
    pending = false;
    return final_status;
  }
};

struct WaitableAwaiter {
  uint32_t waitable;
  uint32_t (*cancel_fn)(uint32_t) = nullptr;
  uint32_t code = 0;
  AsyncTaskContext *ctx = nullptr;
  std::coroutine_handle<> handle = nullptr;
  bool pending = false;

  WaitableAwaiter(uint32_t w, uint32_t (*cf)(uint32_t) = nullptr)
      : waitable(w), cancel_fn(cf) {}
  WaitableAwaiter(const WaitableAwaiter &) = delete;
  WaitableAwaiter &operator=(const WaitableAwaiter &) = delete;

  ~WaitableAwaiter() {
    if (pending && ctx) {
      ctx->cancel_waitable(waitable, handle);
    }
  }

  bool await_ready() const noexcept { return false; }
  void await_suspend(std::coroutine_handle<> h) noexcept {
    ctx = ensure_current_context();
    handle = h;
    pending = true;
    ctx->register_waitable(
        waitable, h, &code, /*is_subtask=*/false, cancel_fn);
  }
  uint32_t await_resume() noexcept {
    pending = false;
    return code;
  }
};

#ifdef WIT_SYMMETRIC
// Let the executor run other callbacks before continuing.
struct YieldAwaiter {
  uint8_t *generator = nullptr;
  EventAwaiter *awaiter = nullptr;

  YieldAwaiter() = default;
  YieldAwaiter(const YieldAwaiter &) = delete;
  YieldAwaiter &operator=(const YieldAwaiter &) = delete;
  ~YieldAwaiter() {
    delete awaiter;
    if (generator) {
      WIT_SYM_EXE(X5Bresource_dropX5Devent_generator)(generator);
    }
  }

  bool await_ready() const noexcept { return false; }
  bool await_suspend(std::coroutine_handle<> h) noexcept {
    generator = WIT_SYM_EXE(X5BconstructorX5Devent_generator)();
    uint8_t *event = WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Esubscribe)(generator);
    awaiter = new EventAwaiter(event);
    // not active yet, so this registers the callback without running it
    bool suspend = awaiter->await_suspend(h);
    if (suspend) {
      WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Eactivate)(generator);
    }
    return suspend;
  }
  void await_resume() noexcept {
    if (awaiter) {
      awaiter->await_resume();
    }
  }
};
#else
struct YieldAwaiter {
  AsyncTaskContext *ctx = nullptr;
  std::coroutine_handle<> handle = nullptr;
  bool pending = false;

  YieldAwaiter() = default;
  YieldAwaiter(const YieldAwaiter &) = delete;
  YieldAwaiter &operator=(const YieldAwaiter &) = delete;

  ~YieldAwaiter() {
    if (pending && ctx) {
      ctx->cancel_ready(handle);
    }
  }

  bool await_ready() const noexcept { return false; }
  void await_suspend(std::coroutine_handle<> h) noexcept {
    ctx = ensure_current_context();
    handle = h;
    pending = true;
    ctx->should_yield = true;
    ctx->ready_queue.push_back(h);
  }
  void await_resume() noexcept {
    pending = false;
  }
};

#endif // WIT_SYMMETRIC

#ifdef WIT_SYMMETRIC
/// Let the executor run until `task` has finished.
template <class TaskT> inline void block_on_task(TaskT &task) {
  uint8_t *generator = WIT_SYM_EXE(X5BconstructorX5Devent_generator)();
  uint8_t *event = WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Esubscribe)(generator);
  bool done = false;
  [](TaskT &t, uint8_t *gen, bool *finished) -> Detached {
    co_await t;
    *finished = true;
    WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Eactivate)(gen);
  }(task, generator, &done);
  if (done) {
    WIT_SYM_EXE(X5Bresource_dropX5Devent_subscription)(event);
  } else {
    WIT_SYM_EXE(block_on)(event);
  }
  WIT_SYM_EXE(X5Bresource_dropX5Devent_generator)(generator);
}
#endif

inline void drive_task_synchronously(std::coroutine_handle<> handle) {
  if (!handle || handle.done()) {
    return;
  }
  auto *&ctx = current_context();
  if (!ctx) {
    ctx = new AsyncTaskContext();
  }
  ctx->owns_root = false;
  ctx->root_handle = handle;
  uint32_t code = ctx->step();
  while (code != CALLBACK_CODE_EXIT && !ctx->is_done()) {
    if (code == CALLBACK_CODE_YIELD) {
      wit_async_thread_yield();
      code = ctx->step();
    } else {
      uint32_t ws = code >> 4;
      uint32_t payload[2] = {0, 0};
      uint32_t ev0 = wit_async_waitable_set_wait(ws, payload);
      ctx->deliver_event(ev0, payload[0], payload[1]);
      code = ctx->step();
    }
  }
  delete ctx;
  ctx = nullptr;
}

} // namespace detail

inline detail::YieldAwaiter yield_async() noexcept { return {}; }

inline bool yield_blocking() { return detail::wit_async_thread_yield() == 0; }

inline void backpressure_inc() { detail::wit_async_backpressure_inc(); }

inline void backpressure_dec() { detail::wit_async_backpressure_dec(); }

/// @brief Represents an ongoing asynchronous computation in C++20 coroutines.
template <typename T = void>
class Task {
public:
  struct promise_type;
  using handle_type = std::coroutine_handle<promise_type>;

  struct promise_type {
    std::optional<T> value;
    std::coroutine_handle<> continuation = nullptr;

    Task get_return_object() {
      return Task(handle_type::from_promise(*this));
    }
    std::suspend_never initial_suspend() noexcept { return {}; }
    auto final_suspend() noexcept {
      struct FinalAwaiter {
        bool await_ready() noexcept { return false; }
        std::coroutine_handle<> await_suspend(handle_type h) noexcept {
          auto cont = h.promise().continuation;
          if (cont) return cont;
          return std::noop_coroutine();
        }
        void await_resume() noexcept {}
      };
      return FinalAwaiter{};
    }
    void return_value(T v) {
      value.emplace(std::move(v));
    }
    void unhandled_exception() {
      assert(false && "unhandled exception in wit::Task");
    }
  };

  handle_type handle = nullptr;

  Task() = default;
  explicit Task(handle_type h) : handle(h) {}
  Task(Task &&other) noexcept : handle(other.handle) { other.handle = nullptr; }
  Task &operator=(Task &&other) noexcept {
    if (this != &other) {
      if (handle) handle.destroy();
      handle = other.handle;
      other.handle = nullptr;
    }
    return *this;
  }
  ~Task() {
    if (handle) handle.destroy();
  }
  Task(const Task &) = delete;
  Task &operator=(const Task &) = delete;

  bool await_ready() const noexcept {
    return !handle || handle.done();
  }
  void await_suspend(std::coroutine_handle<> awaiting) noexcept {
    handle.promise().continuation = awaiting;
  }
  T await_resume() {
    assert(handle && handle.promise().value.has_value());
    return std::move(*handle.promise().value);
  }

  bool done() const noexcept {
    return !handle || handle.done();
  }

  void wait() {
    if (handle && !handle.done()) {
#ifdef WIT_SYMMETRIC
      detail::block_on_task(*this);
#else
      detail::drive_task_synchronously(handle);
#endif
    }
  }

  T get() {
    wait();
    assert(handle && handle.promise().value.has_value());
    return std::move(*handle.promise().value);
  }

  handle_type release() noexcept {
    auto h = handle;
    handle = nullptr;
    return h;
  }
};

template <>
class Task<void> {
public:
  struct promise_type;
  using handle_type = std::coroutine_handle<promise_type>;

  struct promise_type {
    std::coroutine_handle<> continuation = nullptr;

    Task get_return_object() {
      return Task(handle_type::from_promise(*this));
    }
    std::suspend_never initial_suspend() noexcept { return {}; }
    auto final_suspend() noexcept {
      struct FinalAwaiter {
        bool await_ready() noexcept { return false; }
        std::coroutine_handle<> await_suspend(handle_type h) noexcept {
          auto cont = h.promise().continuation;
          if (cont) return cont;
          return std::noop_coroutine();
        }
        void await_resume() noexcept {}
      };
      return FinalAwaiter{};
    }
    void return_void() {}
    void unhandled_exception() {
      assert(false && "unhandled exception in wit::Task");
    }
  };

  handle_type handle = nullptr;

  Task() = default;
  explicit Task(handle_type h) : handle(h) {}
  Task(Task &&other) noexcept : handle(other.handle) { other.handle = nullptr; }
  Task &operator=(Task &&other) noexcept {
    if (this != &other) {
      if (handle) handle.destroy();
      handle = other.handle;
      other.handle = nullptr;
    }
    return *this;
  }
  ~Task() {
    if (handle) handle.destroy();
  }
  Task(const Task &) = delete;
  Task &operator=(const Task &) = delete;

  bool await_ready() const noexcept {
    return !handle || handle.done();
  }
  void await_suspend(std::coroutine_handle<> awaiting) noexcept {
    handle.promise().continuation = awaiting;
  }
  void await_resume() {}

  bool done() const noexcept {
    return !handle || handle.done();
  }

  void wait() {
    if (handle && !handle.done()) {
#ifdef WIT_SYMMETRIC
      detail::block_on_task(*this);
#else
      detail::drive_task_synchronously(handle);
#endif
    }
  }

  void get() { wait(); }

  handle_type release() noexcept {
    auto h = handle;
    handle = nullptr;
    return h;
  }
};

inline void spawn(Task<void> task) {
  if (!task.done()) {
#ifdef WIT_SYMMETRIC
    // continues from executor callbacks, the wrapper cleans up when done
    [](Task<void> t) -> detail::Detached { co_await t; }(std::move(task));
#else
    detail::ensure_current_context()->spawned_handles.push_back(task.release());
#endif
  }
}

template <typename F>
  requires(!std::is_same_v<std::decay_t<F>, Task<void>>)
inline void spawn(F &&func) {
  auto wrapper = [](std::decay_t<F> f) -> Task<void> { co_await f(); };
  spawn(wrapper(std::forward<F>(func)));
}

namespace detail {

#ifdef WIT_SYMMETRIC
/// State of a call of an async export using the symmetric ABI
struct ExportTask {
  Task<void> task;
  uint8_t *generator = nullptr;
  // still in the initial call (the caller doesn't need to wait if we are done)
  bool initial_call = true;

  /// The results have been written to the caller's buffer
  void complete() {
    if (!initial_call) {
      WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Eactivate)(generator);
    }
  }

  static uint8_t cleanup(void *data) {
    auto *self = static_cast<ExportTask *>(data);
    // the coroutine is suspended at its end by now
    WIT_SYM_EXE(X5Bresource_dropX5Devent_generator)(self->generator);
    delete self;
    return 1; // CallbackState::ready
  }
};

/// Start an async export, `make_task` is called with the `ExportTask` to
/// report completion to (`ExportTask::complete`) and returns the coroutine.
/// Returns null if the call has completed, otherwise the event subscription
/// which triggers when the coroutine completes (and has written the results).
template <typename F> inline uint8_t *start_task_symmetric(F &&make_task) {
  auto *state = new ExportTask;
  state->task = make_task(state);
  if (state->task.done()) {
    delete state;
    return nullptr;
  }
  state->initial_call = false;
  state->generator = WIT_SYM_EXE(X5BconstructorX5Devent_generator)();
  uint8_t *result = WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Esubscribe)(state->generator);
  // free the state after the completion has been signalled
  uint8_t *cleanup_event = WIT_SYM_EXE(X5BmethodX5Devent_generatorX2Esubscribe)(state->generator);
  sym_register(cleanup_event, &ExportTask::cleanup, state);
  return result;
}
#endif

template <typename F>
inline int32_t start_task(F &&make_task) {
  AsyncTaskContext *prev_ctx = current_context();
  current_context() = nullptr;
  void *saved_slot = wit_async_context_get();
  wit_async_context_set(nullptr);

  Task<void> task = make_task();
  AsyncTaskContext *ctx = current_context();

  if (!ctx) {
    assert(task.done());
    current_context() = prev_ctx;
    wit_async_context_set(saved_slot);
    return static_cast<int32_t>(CALLBACK_CODE_EXIT);
  }

  ctx->root_handle = task.release();
  uint32_t code = ctx->step();
  current_context() = prev_ctx;

  if (code == CALLBACK_CODE_EXIT) {
    delete ctx;
    wit_async_context_set(saved_slot);
    return static_cast<int32_t>(CALLBACK_CODE_EXIT);
  }

  assert(saved_slot == nullptr);
  wit_async_context_set(ctx);
  return static_cast<int32_t>(code);
}

inline int32_t callback(uint32_t event0, uint32_t event1, uint32_t event2) {
  auto *ctx = static_cast<AsyncTaskContext *>(wit_async_context_get());
  assert(ctx != nullptr);
  wit_async_context_set(nullptr);

  AsyncTaskContext *prev_ctx = current_context();
  current_context() = ctx;

  ctx->deliver_event(event0, event1, event2);
  uint32_t code = ctx->step();

  current_context() = prev_ctx;

  if (code == CALLBACK_CODE_EXIT) {
    delete ctx;
    return static_cast<int32_t>(CALLBACK_CODE_EXIT);
  }

  wit_async_context_set(ctx);
  return static_cast<int32_t>(code);
}

template <typename T>
struct FutureVtable {
  size_t size;
  size_t align;
  void (*lower)(T value, uint8_t *dst);
  void (*dealloc_lists)(uint8_t *dst);
  T (*lift)(uint8_t *src);
#ifndef WIT_SYMMETRIC // the symmetric ABI only needs the representation
  uint32_t (*start_write)(uint32_t future, const uint8_t *val);
  uint32_t (*start_read)(uint32_t future, uint8_t *val);
  uint32_t (*cancel_write)(uint32_t future);
  uint32_t (*cancel_read)(uint32_t future);
  void (*drop_writable)(uint32_t future);
  void (*drop_readable)(uint32_t future);
  uint64_t (*new_future)();
#endif
};

template <>
struct FutureVtable<void> {
  size_t size;
  size_t align;
  void (*lower)(uint8_t *dst);
  void (*dealloc_lists)(uint8_t *dst);
  void (*lift)(uint8_t *src);
#ifndef WIT_SYMMETRIC // the symmetric ABI only needs the representation
  uint32_t (*start_write)(uint32_t future, const uint8_t *val);
  uint32_t (*start_read)(uint32_t future, uint8_t *val);
  uint32_t (*cancel_write)(uint32_t future);
  uint32_t (*cancel_read)(uint32_t future);
  void (*drop_writable)(uint32_t future);
  void (*drop_readable)(uint32_t future);
  uint64_t (*new_future)();
#endif
};

template <typename T>
struct StreamVtable {
  size_t size;
  size_t align;
  void (*lower)(T value, uint8_t *dst);
  void (*dealloc_lists)(uint8_t *dst);
  T (*lift)(uint8_t *src);
#ifndef WIT_SYMMETRIC // the symmetric ABI only needs the representation
  uint32_t (*start_write)(uint32_t stream, const uint8_t *val, size_t amt);
  uint32_t (*start_read)(uint32_t stream, uint8_t *val, size_t amt);
  uint32_t (*cancel_write)(uint32_t stream);
  uint32_t (*cancel_read)(uint32_t stream);
  void (*drop_writable)(uint32_t stream);
  void (*drop_readable)(uint32_t stream);
  uint64_t (*new_stream)();
#endif
};

template <>
struct StreamVtable<void> {
  size_t size;
  size_t align;
  void (*lower)(uint8_t *dst);
  void (*dealloc_lists)(uint8_t *dst);
  void (*lift)(uint8_t *src);
#ifndef WIT_SYMMETRIC // the symmetric ABI only needs the representation
  uint32_t (*start_write)(uint32_t stream, const uint8_t *val, size_t amt);
  uint32_t (*start_read)(uint32_t stream, uint8_t *val, size_t amt);
  uint32_t (*cancel_write)(uint32_t stream);
  uint32_t (*cancel_read)(uint32_t stream);
  void (*drop_writable)(uint32_t stream);
  void (*drop_readable)(uint32_t stream);
  uint64_t (*new_stream)();
#endif
};

template <typename T>
const FutureVtable<T> *get_future_vtable();
template <>
const FutureVtable<void> *get_future_vtable<void>();

template <typename T>
const StreamVtable<T> *get_stream_vtable();
template <>
const StreamVtable<void> *get_stream_vtable<void>();

} // namespace detail

#ifdef WIT_SYMMETRIC
namespace detail {
// The symmetric stream runtime (see crates/symmetric_executor/wit/stream.wit),
// accessed with plain pointers like the executor above.
#define WIT_SYM_STR(x) symmetricX3AruntimeX2Fsymmetric_streamX400X2E3X2E1X00##x
extern "C" void WIT_SYM_STR(X5Bresource_dropX5Dbuffer)(uint8_t *);
extern "C" uint8_t *WIT_SYM_STR(X5BconstructorX5Dbuffer)(uint8_t *, int64_t);
extern "C" uint8_t *WIT_SYM_STR(X5BmethodX5DbufferX2Eget_address)(uint8_t *);
extern "C" int64_t WIT_SYM_STR(X5BmethodX5DbufferX2Eget_size)(uint8_t *);
extern "C" void WIT_SYM_STR(X5BmethodX5DbufferX2Eset_size)(uint8_t *, int64_t);
extern "C" int64_t WIT_SYM_STR(X5BmethodX5DbufferX2Ecapacity)(uint8_t *);
extern "C" void WIT_SYM_STR(X5Bresource_dropX5Dstream_obj)(uint8_t *);
extern "C" uint8_t *WIT_SYM_STR(X5BconstructorX5Dstream_obj)();
extern "C" uint8_t *WIT_SYM_STR(X5BmethodX5Dstream_objX2Eclone)(uint8_t *, int32_t);
extern "C" void WIT_SYM_STR(X5BmethodX5Dstream_objX2Estart_reading)(uint8_t *, uint8_t *, uint8_t *);
extern "C" uint8_t *WIT_SYM_STR(X5BmethodX5Dstream_objX2Eread_ready_subscribe)(uint8_t *);
extern "C" void WIT_SYM_STR(X5BmethodX5Dstream_objX2Eread_result)(uint8_t *, uint8_t *);
extern "C" void WIT_SYM_STR(X5BmethodX5Dstream_objX2Estart_writing)(uint8_t *, uint8_t *);
extern "C" uint8_t *WIT_SYM_STR(X5BmethodX5Dstream_objX2Ewrite_ready_subscribe)(uint8_t *);
extern "C" void WIT_SYM_STR(X5BmethodX5Dstream_objX2Efinish_writing)(uint8_t *, uint8_t *, uint8_t *);
extern "C" void WIT_SYM_EXE(X5BmethodX5Devent_subscriptionX2Ereset)(uint8_t *);

/// Elements without a payload use this as their (unused) memory
inline uint8_t sym_no_payload[8];

/// The ownership of a stream object, the reader and writer are different
/// objects (`clone`) sharing the state.
inline void sym_stream_drop(uint8_t *stream) {
  WIT_SYM_STR(X5Bresource_dropX5Dstream_obj)(stream);
}

inline uint8_t *sym_buffer_new(void *addr, size_t capacity) {
  return WIT_SYM_STR(X5BconstructorX5Dbuffer)(static_cast<uint8_t *>(addr),
                                              static_cast<int64_t>(capacity));
}
inline uint8_t *sym_buffer_address(uint8_t *buf) {
  return WIT_SYM_STR(X5BmethodX5DbufferX2Eget_address)(buf);
}
inline size_t sym_buffer_size(uint8_t *buf) {
  return static_cast<size_t>(WIT_SYM_STR(X5BmethodX5DbufferX2Eget_size)(buf));
}
inline size_t sym_buffer_capacity(uint8_t *buf) {
  return static_cast<size_t>(WIT_SYM_STR(X5BmethodX5DbufferX2Ecapacity)(buf));
}
inline void sym_buffer_set_size(uint8_t *buf, size_t size) {
  WIT_SYM_STR(X5BmethodX5DbufferX2Eset_size)(buf, static_cast<int64_t>(size));
}
inline void sym_buffer_drop(uint8_t *buf) {
  WIT_SYM_STR(X5Bresource_dropX5Dbuffer)(buf);
}

enum class SymState { Ok, Eof, Pending };
struct SymResult {
  SymState state;
  uint8_t *buffer;
};

/// The data the writer has handed over
inline SymResult sym_read_result(uint8_t *stream) {
  alignas(void *) uint8_t ret[2 * sizeof(void *)];
  WIT_SYM_STR(X5BmethodX5Dstream_objX2Eread_result)(stream, ret);
  if (ret[0] == 0) {
    return {SymState::Ok, *reinterpret_cast<uint8_t **>(ret + sizeof(void *))};
  }
  return {ret[sizeof(void *)] == 0 ? SymState::Eof : SymState::Pending,
          nullptr};
}

/// The buffer a reader is waiting with
inline SymResult sym_start_writing(uint8_t *stream) {
  alignas(void *) uint8_t ret[2 * sizeof(void *)];
  WIT_SYM_STR(X5BmethodX5Dstream_objX2Estart_writing)(stream, ret);
  if (ret[0] == 0) {
    return {SymState::Ok, *reinterpret_cast<uint8_t **>(ret + sizeof(void *))};
  }
  return {ret[sizeof(void *)] == 0 ? SymState::Eof : SymState::Pending,
          nullptr};
}

/// Returns null if the buffer was accepted, otherwise it is handed back
inline uint8_t *sym_start_reading(uint8_t *stream, uint8_t *buffer) {
  alignas(void *) uint8_t ret[2 * sizeof(void *)];
  WIT_SYM_STR(X5BmethodX5Dstream_objX2Estart_reading)(stream, buffer, ret);
  return ret[0] == 0 ? nullptr
                     : *reinterpret_cast<uint8_t **>(ret + sizeof(void *));
}

/// Returns null if the buffer was accepted, otherwise it is handed back
inline uint8_t *sym_finish_writing(uint8_t *stream, uint8_t *buffer) {
  alignas(void *) uint8_t ret[2 * sizeof(void *)];
  WIT_SYM_STR(X5BmethodX5Dstream_objX2Efinish_writing)(stream, buffer, ret);
  return ret[0] == 0 ? nullptr
                     : *reinterpret_cast<uint8_t **>(ret + sizeof(void *));
}

/// A subscription which only reports what happens from now on
inline uint8_t *sym_subscription_new(uint8_t *event) {
  WIT_SYM_EXE(X5BmethodX5Devent_subscriptionX2Ereset)(event);
  return event;
}
inline void sym_subscription_drop(uint8_t *event) {
  WIT_SYM_EXE(X5Bresource_dropX5Devent_subscription)(event);
}

/// Memory for the elements a reader is waiting for. If the reader gives up
/// while the buffer is lent to the writer it must not be freed.
struct SymReadBuffer {
  uint8_t *ptr;
  bool owned;
  bool offered = false;
  explicit SymReadBuffer(size_t bytes)
      : ptr(bytes > 0 ? static_cast<uint8_t *>(malloc(bytes)) : sym_no_payload),
        owned(bytes > 0) {}
  SymReadBuffer(const SymReadBuffer &) = delete;
  SymReadBuffer &operator=(const SymReadBuffer &) = delete;
  ~SymReadBuffer() {
    if (owned && !offered) {
      free(ptr);
    }
  }
};

struct SymRead {
  size_t count; // zero: the writer is gone
  uint8_t *data;
};

/// Wait for the writer to provide up to `max` elements (in `buf` if it
/// uses the buffer we offer)
inline Task<SymRead> sym_read(uint8_t *stream, SymReadBuffer &buf, size_t max) {
  for (;;) {
    uint8_t *event = sym_subscription_new(
        WIT_SYM_STR(X5BmethodX5Dstream_objX2Eread_ready_subscribe)(stream));
    SymResult r = sym_read_result(stream);
    if (r.state == SymState::Ok) {
      sym_subscription_drop(event);
      size_t count = sym_buffer_size(r.buffer);
      uint8_t *data = sym_buffer_address(r.buffer);
      sym_buffer_drop(r.buffer);
      if (data == buf.ptr) {
        buf.offered = false;
      }
      co_return SymRead{count, data};
    }
    if (r.state == SymState::Eof) {
      sym_subscription_drop(event);
      co_return SymRead{0, nullptr};
    }
    if (!buf.offered) {
      uint8_t *rejected = sym_start_reading(stream, sym_buffer_new(buf.ptr, max));
      if (rejected) {
        // an earlier offer is still open, which is as good
        sym_buffer_drop(rejected);
      }
      buf.offered = true;
    }
    co_await EventAwaiter(event);
  }
}

/// Wait for a reader to offer a buffer, null if there are no readers anymore
inline Task<uint8_t *> sym_wait_writable(uint8_t *stream) {
  for (;;) {
    uint8_t *event = sym_subscription_new(
        WIT_SYM_STR(X5BmethodX5Dstream_objX2Ewrite_ready_subscribe)(stream));
    SymResult r = sym_start_writing(stream);
    if (r.state == SymState::Ok) {
      sym_subscription_drop(event);
      co_return r.buffer;
    }
    if (r.state == SymState::Eof) {
      sym_subscription_drop(event);
      co_return nullptr;
    }
    co_await EventAwaiter(event);
  }
}

/// Hand over the first `count` elements (the buffer is consumed)
inline void sym_finish_write(uint8_t *stream, uint8_t *buffer, size_t count) {
  sym_buffer_set_size(buffer, count);
  uint8_t *rejected = sym_finish_writing(stream, buffer);
  if (rejected) {
    // can't happen, the reader's offer was just taken
    sym_buffer_drop(rejected);
  }
}
} // namespace detail

template <typename T = void>
class FutureReader {
public:
  uint8_t *handle = nullptr;
  FutureReader() = default;
  explicit FutureReader(uint8_t *h) : handle(h) {}
  FutureReader(FutureReader &&other) noexcept : handle(other.handle) {
    other.handle = nullptr;
  }
  FutureReader &operator=(FutureReader &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      other.handle = nullptr;
    }
    return *this;
  }
  ~FutureReader() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  FutureReader(const FutureReader &) = delete;
  FutureReader &operator=(const FutureReader &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    return h;
  }

  Task<T> read() {
    assert(handle);
    FutureReader<T> self(into_handle());
    const auto *vt = detail::get_future_vtable<T>();
    detail::SymReadBuffer buf(vt->size);
    detail::SymRead res = co_await detail::sym_read(self.handle, buf, 1);
    // the writer has to provide a value
    if (res.count == 0) {
      abort();
    }
    if constexpr (std::is_void_v<T>) {
      co_return;
    } else {
      co_return vt->lift(res.data);
    }
  }

  Task<T> operator co_await() { return read(); }
};

template <typename T = void>
class FutureWriter {
public:
  uint8_t *handle = nullptr;
  FutureWriter() = default;
  explicit FutureWriter(uint8_t *h) : handle(h) {}
  FutureWriter(FutureWriter &&other) noexcept : handle(other.handle) {
    other.handle = nullptr;
  }
  FutureWriter &operator=(FutureWriter &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      other.handle = nullptr;
    }
    return *this;
  }
  ~FutureWriter() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  FutureWriter(const FutureWriter &) = delete;
  FutureWriter &operator=(const FutureWriter &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    return h;
  }

  /// Returns false if the reader was dropped before the value was delivered
  Task<bool> write(T value) {
    assert(handle);
    FutureWriter<T> self(into_handle());
    const auto *vt = detail::get_future_vtable<T>();
    uint8_t *buffer = co_await detail::sym_wait_writable(self.handle);
    if (!buffer) {
      co_return false;
    }
    vt->lower(std::move(value), detail::sym_buffer_address(buffer));
    detail::sym_finish_write(self.handle, buffer, 1);
    co_return true;
  }
};

template <>
class FutureWriter<void> {
public:
  uint8_t *handle = nullptr;
  FutureWriter() = default;
  explicit FutureWriter(uint8_t *h) : handle(h) {}
  FutureWriter(FutureWriter &&other) noexcept : handle(other.handle) {
    other.handle = nullptr;
  }
  FutureWriter &operator=(FutureWriter &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      other.handle = nullptr;
    }
    return *this;
  }
  ~FutureWriter() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  FutureWriter(const FutureWriter &) = delete;
  FutureWriter &operator=(const FutureWriter &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    return h;
  }

  Task<bool> write() {
    assert(handle);
    FutureWriter<void> self(into_handle());
    uint8_t *buffer = co_await detail::sym_wait_writable(self.handle);
    if (!buffer) {
      co_return false;
    }
    detail::sym_finish_write(self.handle, buffer, 1);
    co_return true;
  }
};

template <typename T = void>
inline std::pair<FutureWriter<T>, FutureReader<T>> future_new() {
  uint8_t *writer = detail::WIT_SYM_STR(X5BconstructorX5Dstream_obj)();
  uint8_t *reader =
      detail::WIT_SYM_STR(X5BmethodX5Dstream_objX2Eclone)(writer, 0);
  return {FutureWriter<T>(writer), FutureReader<T>(reader)};
}
#else
template <typename T = void>
class FutureReader {
public:
  int32_t handle = -1;
  FutureReader() = default;
  explicit FutureReader(int32_t h) : handle(h) {}
  FutureReader(FutureReader &&other) noexcept : handle(other.handle) {
    other.handle = -1;
  }
  FutureReader &operator=(FutureReader &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_future_vtable<T>()->drop_readable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      other.handle = -1;
    }
    return *this;
  }
  ~FutureReader() {
    if (handle >= 0) {
      detail::get_future_vtable<T>()->drop_readable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  FutureReader(const FutureReader &) = delete;
  FutureReader &operator=(const FutureReader &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    return h;
  }
  int32_t into_subtask_handle(
      detail::SubtaskAwaiter::CleanupList &cleanup) {
    int32_t h = into_handle();
    if (h >= 0) {
      cleanup.emplace_back(detail::get_future_vtable<T>()->drop_readable,
                           static_cast<uint32_t>(h));
    }
    return h;
  }

  Task<T> read() {
    assert(handle >= 0);
    FutureReader<T> self(into_handle());
    const auto *vt = detail::get_future_vtable<T>();
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{vt->size > 0 ? static_cast<uint8_t *>(malloc(vt->size)) : nullptr};
    uint32_t h = static_cast<uint32_t>(self.get_handle());
    uint32_t status = vt->start_read(h, buf.ptr);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_read};
    }
    assert((status & 0xfU) == WAITABLE_COMPLETED);
    co_return vt->lift(buf.ptr);
  }

  Task<T> operator co_await() { return read(); }
};

template <>
class FutureReader<void> {
public:
  int32_t handle = -1;
  FutureReader() = default;
  explicit FutureReader(int32_t h) : handle(h) {}
  FutureReader(FutureReader &&other) noexcept : handle(other.handle) {
    other.handle = -1;
  }
  FutureReader &operator=(FutureReader &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_future_vtable<void>()->drop_readable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      other.handle = -1;
    }
    return *this;
  }
  ~FutureReader() {
    if (handle >= 0) {
      detail::get_future_vtable<void>()->drop_readable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  FutureReader(const FutureReader &) = delete;
  FutureReader &operator=(const FutureReader &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    return h;
  }
  int32_t into_subtask_handle(
      detail::SubtaskAwaiter::CleanupList &cleanup) {
    int32_t h = into_handle();
    if (h >= 0) {
      cleanup.emplace_back(detail::get_future_vtable<void>()->drop_readable,
                           static_cast<uint32_t>(h));
    }
    return h;
  }

  Task<void> read() {
    assert(handle >= 0);
    FutureReader<void> self(into_handle());
    const auto *vt = detail::get_future_vtable<void>();
    uint32_t h = static_cast<uint32_t>(self.get_handle());
    uint32_t status = vt->start_read(h, nullptr);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_read};
    }
    assert((status & 0xfU) == WAITABLE_COMPLETED);
    co_return;
  }

  Task<void> operator co_await() { return read(); }
};

template <typename T = void>
class FutureWriter {
public:
  int32_t handle = -1;
  FutureWriter() = default;
  explicit FutureWriter(int32_t h) : handle(h) {}
  FutureWriter(FutureWriter &&other) noexcept : handle(other.handle) {
    other.handle = -1;
  }
  FutureWriter &operator=(FutureWriter &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_future_vtable<T>()->drop_writable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      other.handle = -1;
    }
    return *this;
  }
  ~FutureWriter() {
    if (handle >= 0) {
      detail::get_future_vtable<T>()->drop_writable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  FutureWriter(const FutureWriter &) = delete;
  FutureWriter &operator=(const FutureWriter &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    return h;
  }

  Task<bool> write(T value) {
    assert(handle >= 0);
    FutureWriter<T> self(into_handle());
    const auto *vt = detail::get_future_vtable<T>();
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{vt->size > 0 ? static_cast<uint8_t *>(malloc(vt->size)) : nullptr};
    vt->lower(std::move(value), buf.ptr);
    uint32_t h = static_cast<uint32_t>(self.get_handle());
    uint32_t status = vt->start_write(h, buf.ptr);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
    }
    uint32_t state = status & 0xfU;
    if (state == WAITABLE_COMPLETED) {
      vt->dealloc_lists(buf.ptr);
      co_return true;
    } else {
      T unused = vt->lift(buf.ptr);
      (void)unused;
      co_return false;
    }
  }
};

template <>
class FutureWriter<void> {
public:
  int32_t handle = -1;
  FutureWriter() = default;
  explicit FutureWriter(int32_t h) : handle(h) {}
  FutureWriter(FutureWriter &&other) noexcept : handle(other.handle) {
    other.handle = -1;
  }
  FutureWriter &operator=(FutureWriter &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_future_vtable<void>()->drop_writable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      other.handle = -1;
    }
    return *this;
  }
  ~FutureWriter() {
    if (handle >= 0) {
      detail::get_future_vtable<void>()->drop_writable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  FutureWriter(const FutureWriter &) = delete;
  FutureWriter &operator=(const FutureWriter &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    return h;
  }

  Task<bool> write() {
    assert(handle >= 0);
    FutureWriter<void> self(into_handle());
    const auto *vt = detail::get_future_vtable<void>();
    uint32_t h = static_cast<uint32_t>(self.get_handle());
    uint32_t status = vt->start_write(h, nullptr);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
    }
    uint32_t state = status & 0xfU;
    co_return state == WAITABLE_COMPLETED;
  }
};

template <typename T = void>
inline std::pair<FutureWriter<T>, FutureReader<T>> future_new() {
  const auto *vt = detail::get_future_vtable<T>();
  uint64_t handles = vt->new_future();
  int32_t reader = static_cast<int32_t>(handles & 0xffffffffULL);
  int32_t writer = static_cast<int32_t>(handles >> 32);
  return {FutureWriter<T>(writer), FutureReader<T>(reader)};
}

#endif // WIT_SYMMETRIC

enum class StreamState {
  Complete,
  Dropped,
  Cancelled,
};

struct StreamStatus {
  StreamState state;
  size_t count;
};

#ifdef WIT_SYMMETRIC
template <typename T = void>
class StreamReader {
public:
  uint8_t *handle = nullptr;
  bool peer_dropped = false;
  // elements a writer provided beyond what has been asked for
  std::vector<T> leftover;
  StreamReader() = default;
  explicit StreamReader(uint8_t *h) : handle(h) {}
  StreamReader(StreamReader &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped),
        leftover(std::move(other.leftover)) {
    other.handle = nullptr;
    other.peer_dropped = false;
  }
  StreamReader &operator=(StreamReader &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      leftover = std::move(other.leftover);
      other.handle = nullptr;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamReader() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  StreamReader(const StreamReader &) = delete;
  StreamReader &operator=(const StreamReader &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    peer_dropped = false;
    return h;
  }

  Task<std::optional<T>> next() {
    auto [st, items] = co_await read_chunk(1);
    (void)st;
    if (items.empty()) {
      co_return std::nullopt;
    }
    co_return std::move(items[0]);
  }

  Task<std::pair<StreamStatus, std::vector<T>>> read_chunk(size_t max_count) {
    assert(handle);
    std::vector<T> result;
    if (leftover.empty()) {
      if (peer_dropped) {
        co_return std::make_pair(StreamStatus{StreamState::Dropped, 0},
                                 std::vector<T>{});
      }
      const auto *vt = detail::get_stream_vtable<T>();
      detail::SymReadBuffer buf(vt->size * max_count);
      detail::SymRead res = co_await detail::sym_read(handle, buf, max_count);
      if (res.count == 0) {
        peer_dropped = true;
        co_return std::make_pair(StreamStatus{StreamState::Dropped, 0},
                                 std::vector<T>{});
      }
      result.reserve(res.count);
      for (size_t i = 0; i < res.count; ++i) {
        result.push_back(vt->lift(res.data + i * vt->size));
      }
    } else {
      result = std::move(leftover);
      leftover.clear();
    }
    if (result.size() > max_count) {
      // the writer handed over more than requested
      for (size_t i = max_count; i < result.size(); ++i) {
        leftover.push_back(std::move(result[i]));
      }
      while (result.size() > max_count) {
        result.pop_back();
      }
    }
    size_t amt = result.size();
    co_return std::make_pair(StreamStatus{StreamState::Complete, amt},
                             std::move(result));
  }

  Task<std::vector<T>> read(size_t max_count) {
    auto [st, vec] = co_await read_chunk(max_count);
    (void)st;
    co_return std::move(vec);
  }

  Task<std::vector<T>> collect() {
    std::vector<T> result;
    while (true) {
      auto item = co_await next();
      if (!item.has_value()) {
        break;
      }
      result.push_back(std::move(*item));
    }
    co_return result;
  }
};

template <>
class StreamReader<void> {
public:
  uint8_t *handle = nullptr;
  bool peer_dropped = false;
  size_t leftover = 0;
  StreamReader() = default;
  explicit StreamReader(uint8_t *h) : handle(h) {}
  StreamReader(StreamReader &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped),
        leftover(other.leftover) {
    other.handle = nullptr;
    other.peer_dropped = false;
    other.leftover = 0;
  }
  StreamReader &operator=(StreamReader &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      leftover = other.leftover;
      other.handle = nullptr;
      other.peer_dropped = false;
      other.leftover = 0;
    }
    return *this;
  }
  ~StreamReader() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  StreamReader(const StreamReader &) = delete;
  StreamReader &operator=(const StreamReader &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    peer_dropped = false;
    return h;
  }

  Task<StreamStatus> read(size_t max_count = 1) {
    assert(handle);
    size_t amt = leftover;
    if (amt == 0) {
      if (peer_dropped) {
        co_return StreamStatus{StreamState::Dropped, 0};
      }
      detail::SymReadBuffer buf(0);
      detail::SymRead res = co_await detail::sym_read(handle, buf, max_count);
      if (res.count == 0) {
        peer_dropped = true;
        co_return StreamStatus{StreamState::Dropped, 0};
      }
      amt = res.count;
    }
    leftover = amt > max_count ? amt - max_count : 0;
    amt = amt > max_count ? max_count : amt;
    co_return StreamStatus{StreamState::Complete, amt};
  }

  Task<bool> next() {
    auto st = co_await read(1);
    co_return st.count > 0;
  }
};

template <typename T = void>
class StreamWriter {
public:
  uint8_t *handle = nullptr;
  bool peer_dropped = false;
  StreamWriter() = default;
  explicit StreamWriter(uint8_t *h) : handle(h) {}
  StreamWriter(StreamWriter &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = nullptr;
    other.peer_dropped = false;
  }
  StreamWriter &operator=(StreamWriter &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = nullptr;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamWriter() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  StreamWriter(const StreamWriter &) = delete;
  StreamWriter &operator=(const StreamWriter &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    peer_dropped = false;
    return h;
  }

  /// Writes all of the values unless the reader goes away, the status tells
  /// how many were written. Values which weren't written are dropped.
  Task<StreamStatus> write(std::vector<T> values) {
    assert(handle);
    if (peer_dropped) {
      co_return StreamStatus{StreamState::Dropped, 0};
    }
    const auto *vt = detail::get_stream_vtable<T>();
    size_t count = values.size();
    size_t done = 0;
    while (done < count) {
      uint8_t *buffer = co_await detail::sym_wait_writable(handle);
      if (!buffer) {
        peer_dropped = true;
        break;
      }
      size_t n = detail::sym_buffer_capacity(buffer);
      if (n > count - done) {
        n = count - done;
      }
      uint8_t *dest = detail::sym_buffer_address(buffer);
      for (size_t i = 0; i < n; ++i) {
        vt->lower(std::move(values[done + i]), dest + i * vt->size);
      }
      detail::sym_finish_write(handle, buffer, n);
      done += n;
    }
    StreamState s = done > 0               ? StreamState::Complete
                    : peer_dropped         ? StreamState::Dropped
                                           : StreamState::Cancelled;
    co_return StreamStatus{s, done};
  }

  Task<bool> write_one(T value) {
    std::vector<T> values;
    values.push_back(std::move(value));
    auto st = co_await write(std::move(values));
    co_return st.count > 0;
  }

  Task<size_t> write_all(std::vector<T> values) {
    if (values.empty() || peer_dropped) {
      co_return 0;
    }
    auto st = co_await write(std::move(values));
    co_return st.count;
  }

  Task<size_t> write_all(wit::vector<T> values) {
    std::vector<T> vec;
    vec.reserve(values.size());
    for (size_t i = 0; i < values.size(); ++i) {
      vec.push_back(std::move(values[i]));
    }
    co_return co_await write_all(std::move(vec));
  }
};

template <>
class StreamWriter<void> {
public:
  uint8_t *handle = nullptr;
  bool peer_dropped = false;
  StreamWriter() = default;
  explicit StreamWriter(uint8_t *h) : handle(h) {}
  StreamWriter(StreamWriter &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = nullptr;
    other.peer_dropped = false;
  }
  StreamWriter &operator=(StreamWriter &&other) noexcept {
    if (this != &other) {
      if (handle) {
        detail::sym_stream_drop(handle);
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = nullptr;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamWriter() {
    if (handle) {
      detail::sym_stream_drop(handle);
      handle = nullptr;
    }
  }
  StreamWriter(const StreamWriter &) = delete;
  StreamWriter &operator=(const StreamWriter &) = delete;

  uint8_t *get_handle() const { return handle; }
  uint8_t *into_handle() {
    uint8_t *h = handle;
    handle = nullptr;
    peer_dropped = false;
    return h;
  }

  Task<StreamStatus> write(size_t count = 1) {
    assert(handle);
    if (peer_dropped) {
      co_return StreamStatus{StreamState::Dropped, 0};
    }
    size_t done = 0;
    while (done < count) {
      uint8_t *buffer = co_await detail::sym_wait_writable(handle);
      if (!buffer) {
        peer_dropped = true;
        break;
      }
      size_t n = detail::sym_buffer_capacity(buffer);
      if (n > count - done) {
        n = count - done;
      }
      detail::sym_finish_write(handle, buffer, n);
      done += n;
    }
    StreamState s = done > 0               ? StreamState::Complete
                    : peer_dropped         ? StreamState::Dropped
                                           : StreamState::Cancelled;
    co_return StreamStatus{s, done};
  }

  Task<bool> write_one() {
    auto st = co_await write(1);
    co_return st.count > 0;
  }
};

template <typename T = void>
inline std::pair<StreamWriter<T>, StreamReader<T>> stream_new() {
  uint8_t *writer = detail::WIT_SYM_STR(X5BconstructorX5Dstream_obj)();
  uint8_t *reader =
      detail::WIT_SYM_STR(X5BmethodX5Dstream_objX2Eclone)(writer, 0);
  return {StreamWriter<T>(writer), StreamReader<T>(reader)};
}
#else
template <typename T = void>
class StreamReader {
public:
  int32_t handle = -1;
  bool peer_dropped = false;
  StreamReader() = default;
  explicit StreamReader(int32_t h) : handle(h) {}
  StreamReader(StreamReader &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = -1;
    other.peer_dropped = false;
  }
  StreamReader &operator=(StreamReader &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_stream_vtable<T>()->drop_readable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = -1;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamReader() {
    if (handle >= 0) {
      detail::get_stream_vtable<T>()->drop_readable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  StreamReader(const StreamReader &) = delete;
  StreamReader &operator=(const StreamReader &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    peer_dropped = false;
    return h;
  }
  int32_t into_subtask_handle(
      detail::SubtaskAwaiter::CleanupList &cleanup) {
    int32_t h = into_handle();
    if (h >= 0) {
      cleanup.emplace_back(detail::get_stream_vtable<T>()->drop_readable,
                           static_cast<uint32_t>(h));
    }
    return h;
  }

  Task<std::optional<T>> next() {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return std::nullopt;
    }
    const auto *vt = detail::get_stream_vtable<T>();
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{vt->size > 0 ? static_cast<uint8_t *>(malloc(vt->size)) : nullptr};
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_read(h, buf.ptr, 1);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_read};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    if (amt == 0 && (state == WAITABLE_DROPPED || state == WAITABLE_CANCELLED)) {
      co_return std::nullopt;
    }
    assert(amt == 1);
    co_return vt->lift(buf.ptr);
  }

  Task<std::pair<StreamStatus, std::vector<T>>> read_chunk(size_t max_count) {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return std::make_pair(StreamStatus{StreamState::Dropped, 0},
                               std::vector<T>{});
    }
    const auto *vt = detail::get_stream_vtable<T>();
    size_t alloc_size = vt->size * max_count;
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{alloc_size > 0 ? static_cast<uint8_t *>(malloc(alloc_size)) : nullptr};
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_read(h, buf.ptr, max_count);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_read};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    StreamState s = amt > 0                     ? StreamState::Complete
                    : state == WAITABLE_DROPPED ? StreamState::Dropped
                                                : StreamState::Cancelled;
    std::vector<T> result;
    result.reserve(amt);
    for (uint32_t i = 0; i < amt; ++i) {
      result.push_back(vt->lift(buf.ptr + i * vt->size));
    }
    co_return std::make_pair(StreamStatus{s, amt}, std::move(result));
  }

  Task<std::vector<T>> read(size_t max_count) {
    auto [st, vec] = co_await read_chunk(max_count);
    (void)st;
    co_return std::move(vec);
  }

  Task<std::vector<T>> collect() {
    std::vector<T> result;
    while (true) {
      auto item = co_await next();
      if (!item.has_value()) {
        break;
      }
      result.push_back(std::move(*item));
    }
    co_return result;
  }
};

template <>
class StreamReader<void> {
public:
  int32_t handle = -1;
  bool peer_dropped = false;
  StreamReader() = default;
  explicit StreamReader(int32_t h) : handle(h) {}
  StreamReader(StreamReader &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = -1;
    other.peer_dropped = false;
  }
  StreamReader &operator=(StreamReader &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_stream_vtable<void>()->drop_readable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = -1;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamReader() {
    if (handle >= 0) {
      detail::get_stream_vtable<void>()->drop_readable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  StreamReader(const StreamReader &) = delete;
  StreamReader &operator=(const StreamReader &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    peer_dropped = false;
    return h;
  }
  int32_t into_subtask_handle(
      detail::SubtaskAwaiter::CleanupList &cleanup) {
    int32_t h = into_handle();
    if (h >= 0) {
      cleanup.emplace_back(detail::get_stream_vtable<void>()->drop_readable,
                           static_cast<uint32_t>(h));
    }
    return h;
  }

  Task<StreamStatus> read(size_t max_count = 1) {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return StreamStatus{StreamState::Dropped, 0};
    }
    const auto *vt = detail::get_stream_vtable<void>();
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_read(h, nullptr, max_count);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_read};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    StreamState s = amt > 0                     ? StreamState::Complete
                    : state == WAITABLE_DROPPED ? StreamState::Dropped
                                                : StreamState::Cancelled;
    co_return StreamStatus{s, amt};
  }

  Task<bool> next() {
    auto st = co_await read(1);
    co_return st.count > 0;
  }
};

template <typename T = void>
class StreamWriter {
public:
  int32_t handle = -1;
  bool peer_dropped = false;
  StreamWriter() = default;
  explicit StreamWriter(int32_t h) : handle(h) {}
  StreamWriter(StreamWriter &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = -1;
    other.peer_dropped = false;
  }
  StreamWriter &operator=(StreamWriter &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_stream_vtable<T>()->drop_writable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = -1;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamWriter() {
    if (handle >= 0) {
      detail::get_stream_vtable<T>()->drop_writable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  StreamWriter(const StreamWriter &) = delete;
  StreamWriter &operator=(const StreamWriter &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    peer_dropped = false;
    return h;
  }

  Task<StreamStatus> write(std::vector<T> values) {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return StreamStatus{StreamState::Dropped, 0};
    }
    const auto *vt = detail::get_stream_vtable<T>();
    size_t count = values.size();
    size_t alloc_size = vt->size * count;
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{alloc_size > 0 ? static_cast<uint8_t *>(malloc(alloc_size)) : nullptr};
    for (size_t i = 0; i < count; ++i) {
      vt->lower(std::move(values[i]), buf.ptr + i * vt->size);
    }
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_write(h, buf.ptr, count);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    for (uint32_t i = 0; i < amt; ++i) {
      vt->dealloc_lists(buf.ptr + i * vt->size);
    }
    for (size_t i = amt; i < count; ++i) {
      T unused = vt->lift(buf.ptr + i * vt->size);
      (void)unused;
    }
    StreamState s = amt > 0                     ? StreamState::Complete
                    : state == WAITABLE_DROPPED ? StreamState::Dropped
                                                : StreamState::Cancelled;
    co_return StreamStatus{s, amt};
  }

  Task<bool> write_one(T value) {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return false;
    }
    const auto *vt = detail::get_stream_vtable<T>();
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{vt->size > 0 ? static_cast<uint8_t *>(malloc(vt->size)) : nullptr};
    vt->lower(std::move(value), buf.ptr);
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_write(h, buf.ptr, 1);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    if (amt == 1) {
      vt->dealloc_lists(buf.ptr);
      co_return true;
    } else {
      T unused = vt->lift(buf.ptr);
      (void)unused;
      co_return false;
    }
  }

  Task<size_t> write_all(std::vector<T> values) {
    assert(handle >= 0);
    if (values.empty() || peer_dropped) {
      co_return 0;
    }
    const auto *vt = detail::get_stream_vtable<T>();
    size_t count = values.size();
    size_t alloc_size = vt->size * count;
    struct BufferGuard {
      uint8_t *ptr;
      ~BufferGuard() {
        if (ptr) free(ptr);
      }
    } buf{alloc_size > 0 ? static_cast<uint8_t *>(malloc(alloc_size)) : nullptr};
    for (size_t i = 0; i < count; ++i) {
      vt->lower(std::move(values[i]), buf.ptr + i * vt->size);
    }
    size_t cursor = 0;
    uint32_t h = static_cast<uint32_t>(handle);
    while (cursor < count) {
      uint32_t status =
          vt->start_write(h, buf.ptr + cursor * vt->size, count - cursor);
      if (status == WAITABLE_STATUS_BLOCKED) {
        status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
      }
      uint32_t state = status & 0xfU;
      uint32_t amt = status >> 4;
      for (uint32_t i = 0; i < amt; ++i) {
        vt->dealloc_lists(buf.ptr + (cursor + i) * vt->size);
      }
      cursor += amt;
      if (state == WAITABLE_DROPPED) {
        peer_dropped = true;
        break;
      }
    }
    for (size_t i = cursor; i < count; ++i) {
      T unused = vt->lift(buf.ptr + i * vt->size);
      (void)unused;
    }
    co_return cursor;
  }

  Task<size_t> write_all(wit::vector<T> values) {
    std::vector<T> vec;
    vec.reserve(values.size());
    for (size_t i = 0; i < values.size(); ++i) {
      vec.push_back(std::move(values[i]));
    }
    co_return co_await write_all(std::move(vec));
  }
};

template <>
class StreamWriter<void> {
public:
  int32_t handle = -1;
  bool peer_dropped = false;
  StreamWriter() = default;
  explicit StreamWriter(int32_t h) : handle(h) {}
  StreamWriter(StreamWriter &&other) noexcept
      : handle(other.handle), peer_dropped(other.peer_dropped) {
    other.handle = -1;
    other.peer_dropped = false;
  }
  StreamWriter &operator=(StreamWriter &&other) noexcept {
    if (this != &other) {
      if (handle >= 0) {
        detail::get_stream_vtable<void>()->drop_writable(
            static_cast<uint32_t>(handle));
      }
      handle = other.handle;
      peer_dropped = other.peer_dropped;
      other.handle = -1;
      other.peer_dropped = false;
    }
    return *this;
  }
  ~StreamWriter() {
    if (handle >= 0) {
      detail::get_stream_vtable<void>()->drop_writable(
          static_cast<uint32_t>(handle));
      handle = -1;
    }
  }
  StreamWriter(const StreamWriter &) = delete;
  StreamWriter &operator=(const StreamWriter &) = delete;

  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    peer_dropped = false;
    return h;
  }

  Task<StreamStatus> write(size_t count = 1) {
    assert(handle >= 0);
    if (peer_dropped) {
      co_return StreamStatus{StreamState::Dropped, 0};
    }
    const auto *vt = detail::get_stream_vtable<void>();
    uint32_t h = static_cast<uint32_t>(handle);
    uint32_t status = vt->start_write(h, nullptr, count);
    if (status == WAITABLE_STATUS_BLOCKED) {
      status = co_await detail::WaitableAwaiter{h, vt->cancel_write};
    }
    uint32_t state = status & 0xfU;
    uint32_t amt = status >> 4;
    if (state == WAITABLE_DROPPED) {
      peer_dropped = true;
    }
    StreamState s = amt > 0                     ? StreamState::Complete
                    : state == WAITABLE_DROPPED ? StreamState::Dropped
                                                : StreamState::Cancelled;
    co_return StreamStatus{s, amt};
  }

  Task<bool> write_one() {
    auto st = co_await write(1);
    co_return st.count > 0;
  }
};

template <typename T = void>
inline std::pair<StreamWriter<T>, StreamReader<T>> stream_new() {
  const auto *vt = detail::get_stream_vtable<T>();
  uint64_t handles = vt->new_stream();
  int32_t reader = static_cast<int32_t>(handles & 0xffffffffULL);
  int32_t writer = static_cast<int32_t>(handles >> 32);
  return {StreamWriter<T>(writer), StreamReader<T>(reader)};
}

#endif // WIT_SYMMETRIC

#endif // __cpp_impl_coroutine

class ErrorContext {
public:
  int32_t handle = -1;
  ErrorContext() = default;
  explicit ErrorContext(int32_t h) : handle(h) {}
  int32_t get_handle() const { return handle; }
  int32_t into_handle() {
    int32_t h = handle;
    handle = -1;
    return h;
  }
};
} // namespace wit
#endif // WIT_BINDGEN_WIT_H_DEFINED
