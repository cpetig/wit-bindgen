#pragma once
// std::expected (plain) and ara::core::Result (--autosar) have different
// accessors, this compiles as C++17
#ifdef WIT_ARA_SPAN
#include <ara/core/result.h>
#else
#include <expected>
#endif
#include <utility>

namespace wit {
namespace detail {
// prefer the AUTOSAR spelling if it exists (the int/long overloads order
// the candidates)
template <class R> auto result_ok_impl(const R &r, int) -> decltype(r.HasValue()) {
  return r.HasValue();
}
template <class R> auto result_ok_impl(const R &r, long) -> decltype(r.has_value()) {
  return r.has_value();
}
template <class R>
auto result_value_impl(R &&r, int) -> decltype(std::move(r).Value()) {
  return std::move(r).Value();
}
template <class R>
auto result_value_impl(R &&r, long) -> decltype(std::move(r).value()) {
  return std::move(r).value();
}
template <class R> auto result_error_impl(const R &r, int) -> decltype(r.Error()) {
  return r.Error();
}
template <class R> auto result_error_impl(const R &r, long) -> decltype(r.error()) {
  return r.error();
}
} // namespace detail

template <class R> bool result_ok(const R &r) {
  return detail::result_ok_impl(r, 0);
}
/// move the value out of a successful result
template <class R> auto result_value(R &&r) {
  return detail::result_value_impl(std::forward<R>(r), 0);
}
template <class R> decltype(auto) result_error(const R &r) {
  return detail::result_error_impl(r, 0);
}
} // namespace wit
