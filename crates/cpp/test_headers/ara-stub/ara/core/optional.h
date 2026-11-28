#pragma once
// STUB of ara::core::Optional for compile tests of `--autosar`, see STUB_README.txt
// Like the real one it is not std::optional: no value(), only operator*/->.
#include <optional>
#include <utility>
namespace ara::core {
struct nullopt_t {
  explicit constexpr nullopt_t(int) {}
};
inline constexpr nullopt_t nullopt{0};
template <class T> class Optional {
  std::optional<T> o_;

public:
  constexpr Optional() noexcept = default;
  constexpr Optional(nullopt_t) noexcept {}
  constexpr Optional(T const &v) : o_(v) {}
  constexpr Optional(T &&v) : o_(std::move(v)) {}
  template <class... A> T &emplace(A &&...args) { return o_.emplace(std::forward<A>(args)...); }
  constexpr T const &operator*() const & noexcept { return *o_; }
  constexpr T &operator*() & noexcept { return *o_; }
  constexpr T &&operator*() && noexcept { return std::move(*o_); }
  constexpr T const *operator->() const noexcept { return &*o_; }
  constexpr T *operator->() noexcept { return &*o_; }
  constexpr bool has_value() const noexcept { return o_.has_value(); }
  constexpr explicit operator bool() const noexcept { return o_.has_value(); }
};
} // namespace ara::core
