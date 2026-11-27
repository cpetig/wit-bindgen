#pragma once
// std::expected (plain) and ara::core::Result (--autosar) have different accessors
#include <expected>
#include <utility>

namespace wit {
template <class R> bool result_ok(const R &r) {
  if constexpr (requires { r.HasValue(); })
    return r.HasValue();
  else
    return r.has_value();
}
/// move the value out of a successful result
template <class R> auto result_value(R &&r) {
  if constexpr (requires { std::move(r).Value(); })
    return std::move(r).Value();
  else
    return std::move(r).value();
}
template <class R> decltype(auto) result_error(const R &r) {
  if constexpr (requires { r.Error(); })
    return r.Error();
  else
    return r.error();
}
} // namespace wit
