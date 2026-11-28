#pragma once
// STUB of ara::core::Result for compile tests of `--autosar`, see STUB_README.txt
#include "error_code.h"
#include <type_traits>
#include <utility>
#include <variant>
namespace ara::core {
template <class T, class E = ErrorCode> class Result {
  std::variant<T, E> v_;
  explicit Result(std::in_place_index_t<0>, T &&t) : v_(std::in_place_index<0>, std::move(t)) {}
  explicit Result(std::in_place_index_t<1>, E &&e) : v_(std::in_place_index<1>, std::move(e)) {}

public:
  Result(T const &t) : v_(std::in_place_index<0>, t) {}
  explicit Result(T &&t) : v_(std::in_place_index<0>, std::move(t)) {}
  explicit Result(E const &e) : v_(std::in_place_index<1>, e) {}
  explicit Result(E &&e) : v_(std::in_place_index<1>, std::move(e)) {}
  static Result FromValue(T const &t) { T c(t); return Result(std::in_place_index<0>, std::move(c)); }
  static Result FromValue(T &&t) { return Result(std::in_place_index<0>, std::move(t)); }
  static Result FromError(E const &e) { E c(e); return Result(std::in_place_index<1>, std::move(c)); }
  static Result FromError(E &&e) { return Result(std::in_place_index<1>, std::move(e)); }
  bool HasValue() const noexcept { return v_.index() == 0; }
  explicit operator bool() const noexcept { return HasValue(); }
  T const &operator*() const & { return Value(); }
  T const &Value() const & { return std::get<0>(v_); }
  T &Value() & { return std::get<0>(v_); }
  T &&Value() && { return std::get<0>(std::move(v_)); }
  E const &Error() const & { return std::get<1>(v_); }
  E &&Error() && { return std::get<1>(std::move(v_)); }
};
template <class E> class Result<void, E> {
  std::variant<std::monostate, E> v_;
  explicit Result(std::in_place_index_t<0>) : v_(std::in_place_index<0>) {}
  explicit Result(std::in_place_index_t<1>, E &&e) : v_(std::in_place_index<1>, std::move(e)) {}

public:
  Result() noexcept : v_(std::in_place_index<0>) {}
  explicit Result(E const &e) : v_(std::in_place_index<1>, e) {}
  explicit Result(E &&e) : v_(std::in_place_index<1>, std::move(e)) {}
  static Result FromValue() { return Result(std::in_place_index<0>); }
  static Result FromError(E const &e) { E c(e); return Result(std::in_place_index<1>, std::move(c)); }
  static Result FromError(E &&e) { return Result(std::in_place_index<1>, std::move(e)); }
  bool HasValue() const noexcept { return v_.index() == 0; }
  explicit operator bool() const noexcept { return HasValue(); }
  void operator*() const {}
  void Value() const {}
  E const &Error() const & { return std::get<1>(v_); }
  E &&Error() && { return std::get<1>(std::move(v_)); }
};
} // namespace ara::core
