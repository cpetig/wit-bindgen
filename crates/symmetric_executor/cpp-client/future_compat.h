#pragma once
// The future/promise/thread primitives of the C++17 async API
// (`--async-api future`) in one place, so that std::future/std::promise
// (default) and ara::core::Future/ara::core::Promise (`--autosar`, which makes
// the generated header define WIT_ARA_FUTURE) are interchangeable.
//
// ara::core::Future differs from std::future: it has no exceptions (`get()`
// is replaced by `GetResult()` which returns an ara::core::Result), readiness
// is queried via `is_ready()` and there is no std::async equivalent, so
// threads are started with std::thread directly.
#include "result_compat.h"
#include <atomic>
#include <chrono>
#include <cstdlib>
#include <memory>
#include <mutex>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>
#ifdef WIT_ARA_FUTURE
#include <ara/core/future.h>
#include <ara/core/promise.h>
#else
#include <future>
#endif

namespace wit {

#ifdef WIT_ARA_FUTURE
template <class T> using future = ::ara::core::Future<T>;
template <class T> using promise = ::ara::core::Promise<T>;
#else
template <class T> using future = std::future<T>;
template <class T> using promise = std::promise<T>;
#endif

/// Whether the result of the future is available (get/GetResult won't block)
template <class T> bool future_ready(future<T> &fut) {
#ifdef WIT_ARA_FUTURE
  return fut.is_ready();
#else
  return fut.wait_for(std::chrono::seconds::zero()) == std::future_status::ready;
#endif
}

/// Waits for the future and takes its value (nothing for future<void>),
/// ara::core has no exceptions, so an error result aborts
template <class T> T future_get(future<T> &fut) {
#ifdef WIT_ARA_FUTURE
  auto result = fut.GetResult();
  if (!wit::result_ok(result))
    std::abort();
  if constexpr (std::is_void_v<T>) {
    return;
  } else {
    return wit::result_value(std::move(result));
  }
#else
  return fut.get();
#endif
}

/// A background thread which can be joined by an executor callback. The
/// destructor also joins.
class task {
#ifdef WIT_ARA_FUTURE
  std::thread thread_;

public:
  template <class F, class... A>
  explicit task(F &&fun, A &&...args)
      : thread_(std::forward<F>(fun), std::forward<A>(args)...) {}
  void join() {
    if (thread_.joinable())
      thread_.join();
  }
  ~task() { join(); }
#else
  std::future<void> done_;

public:
  template <class F, class... A>
  explicit task(F &&fun, A &&...args)
      : done_(std::async(std::launch::async, std::forward<F>(fun),
                         std::forward<A>(args)...)) {}
  /// also rethrows an exception of the thread
  void join() {
    if (done_.valid())
      done_.get();
  }
#endif
  task(task const &) = delete;
  task &operator=(task const &) = delete;
};

/// Runs `fun(args...)` on a new thread
template <class F, class... A>
std::unique_ptr<task> spawn(F &&fun, A &&...args) {
  return std::make_unique<task>(std::forward<F>(fun),
                                std::forward<A>(args)...);
}

namespace detail {
#ifdef WIT_ARA_FUTURE
/// Threads behind the futures of async_call, finished ones are joined when a
/// new one is added, the remaining ones when the library is unloaded.
class ThreadRegistry {
  struct Entry {
    std::thread thread;
    std::shared_ptr<std::atomic<bool>> finished;
  };
  std::mutex lock_;
  std::vector<Entry> threads_;

  static void join_all(std::vector<Entry> &entries) {
    for (auto &entry : entries)
      entry.thread.join();
  }

public:
  void add(std::thread &&thread, std::shared_ptr<std::atomic<bool>> finished) {
    std::vector<Entry> done;
    {
      std::lock_guard<std::mutex> guard(lock_);
      std::vector<Entry> running;
      for (auto &entry : threads_) {
        (entry.finished->load() ? done : running).push_back(std::move(entry));
      }
      running.push_back(Entry{std::move(thread), std::move(finished)});
      threads_.swap(running);
    }
    join_all(done);
  }
  ~ThreadRegistry() {
    std::vector<Entry> remaining;
    {
      std::lock_guard<std::mutex> guard(lock_);
      remaining.swap(threads_);
    }
    join_all(remaining);
  }
};
inline ThreadRegistry thread_registry;
#endif
} // namespace detail

/// Calls `fun()` on a new thread, the future becomes ready with its result
/// (std::async replacement)
template <class R, class F> future<R> async_call(F &&fun) {
#ifdef WIT_ARA_FUTURE
  promise<R> result;
  future<R> fut = result.get_future();
  auto finished = std::make_shared<std::atomic<bool>>(false);
  std::thread thread(
      [result = std::move(result), fun = std::forward<F>(fun),
       finished]() mutable {
        if constexpr (std::is_void_v<R>) {
          fun();
          result.set_value();
        } else {
          result.set_value(fun());
        }
        finished->store(true);
      });
  detail::thread_registry.add(std::move(thread), std::move(finished));
  return fut;
#else
  return std::async(std::launch::async, std::forward<F>(fun));
#endif
}

} // namespace wit
