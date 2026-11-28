#pragma once
// STUB of ara::core::Future for compile and smoke tests of `--autosar`, see
// STUB_README.txt. The subset of AUTOSAR R22-11: valid, get, GetResult,
// is_ready, wait, wait_for, then (no exceptions, errors are in the Result).
#include "error_code.h"
#include "result.h"
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <memory>
#include <thread>
#include <mutex>
#include <optional>
#include <type_traits>
#include <utility>
namespace ara::core {
enum class future_status : std::uint8_t { ready, timeout };
template <class T, class E> class Promise;

namespace stub_detail {
template <class T, class E> struct SharedState {
  std::mutex lock;
  std::condition_variable cv;
  std::optional<Result<T, E>> result;
  void set(Result<T, E> &&r) {
    {
      std::lock_guard<std::mutex> guard(lock);
      result.emplace(std::move(r));
    }
    cv.notify_all();
  }
};
} // namespace stub_detail

template <class T, class E = ErrorCode> class Future {
  std::shared_ptr<stub_detail::SharedState<T, E>> state_;
  friend class Promise<T, E>;
  explicit Future(std::shared_ptr<stub_detail::SharedState<T, E>> s) : state_(std::move(s)) {}

public:
  using ValueType = T;
  using ErrorType = E;
  Future() noexcept = default;
  Future(Future &&) noexcept = default;
  Future &operator=(Future &&) noexcept = default;
  Future(Future const &) = delete;
  Future &operator=(Future const &) = delete;

  bool valid() const noexcept { return state_ != nullptr; }
  bool is_ready() const {
    std::lock_guard<std::mutex> guard(state_->lock);
    return state_->result.has_value();
  }
  void wait() const {
    std::unique_lock<std::mutex> guard(state_->lock);
    state_->cv.wait(guard, [&] { return state_->result.has_value(); });
  }
  template <class Rep, class Period>
  future_status wait_for(std::chrono::duration<Rep, Period> const &d) const {
    std::unique_lock<std::mutex> guard(state_->lock);
    return state_->cv.wait_for(guard, d, [&] { return state_->result.has_value(); })
               ? future_status::ready
               : future_status::timeout;
  }
  /// blocks until the result is available and invalidates the future
  Result<T, E> GetResult() {
    wait();
    auto state = std::move(state_);
    return std::move(*state->result);
  }
  T get() {
    auto r = GetResult();
    if constexpr (std::is_void_v<T>) {
      r.Value();
    } else {
      return std::move(r).Value();
    }
  }
  /// calls `fun(Future<T,E>)` once the result is available (on its own thread in
  /// this stub), returns a future of the result of fun
  template <class F> auto then(F &&fun) -> Future<std::invoke_result_t<F, Future<T, E>>, E>;
};

template <class T, class E = ErrorCode> class Promise {
  std::shared_ptr<stub_detail::SharedState<T, E>> state_ = std::make_shared<stub_detail::SharedState<T, E>>();

public:
  Promise() = default;
  Promise(Promise &&) noexcept = default;
  Promise &operator=(Promise &&) noexcept = default;
  Promise(Promise const &) = delete;
  Promise &operator=(Promise const &) = delete;
  Future<T, E> get_future() { return Future<T, E>(state_); }
  template <class U = T, class = std::enable_if_t<!std::is_void_v<U>>>
  void set_value(U const &v) { T c(v); state_->set(Result<T, E>::FromValue(std::move(c))); }
  template <class U = T, class = std::enable_if_t<!std::is_void_v<U>>>
  void set_value(U &&v) { state_->set(Result<T, E>::FromValue(std::move(v))); }
  template <class U = T, class = std::enable_if_t<std::is_void_v<U>>>
  void set_value() { state_->set(Result<T, E>::FromValue()); }
  void SetError(E &&e) { state_->set(Result<T, E>::FromError(std::move(e))); }
  void SetError(E const &e) { state_->set(Result<T, E>::FromError(e)); }
};

template <class T, class E>
template <class F>
auto Future<T, E>::then(F &&fun) -> Future<std::invoke_result_t<F, Future<T, E>>, E> {
  using R = std::invoke_result_t<F, Future<T, E>>;
  Promise<R, E> p;
  auto result = p.get_future();
  std::thread([p = std::move(p), f = std::move(*this), fun = std::forward<F>(fun)]() mutable {
    f.wait();
    if constexpr (std::is_void_v<R>) {
      fun(std::move(f));
      p.set_value();
    } else {
      p.set_value(fun(std::move(f)));
    }
  }).detach();
  return result;
}
} // namespace ara::core
