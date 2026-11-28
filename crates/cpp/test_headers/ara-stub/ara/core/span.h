#pragma once
// STUB of ara::core::Span (dynamic extent only) for compile tests of `--autosar`, see STUB_README.txt
#include <cstddef>
#include <type_traits>
#include <utility>
namespace ara::core {
template <class T> class Span {
  T *data_ = nullptr;
  std::size_t size_ = 0;

public:
  using element_type = T;
  using value_type = std::remove_cv_t<T>;
  using size_type = std::size_t;
  using pointer = T *;
  using reference = T &;
  using iterator = T *;
  constexpr Span() noexcept = default;
  constexpr Span(T *ptr, std::size_t count) noexcept : data_(ptr), size_(count) {}
  template <std::size_t N> constexpr Span(T (&arr)[N]) noexcept : data_(arr), size_(N) {}
  template <class C, class = std::enable_if_t<std::is_convertible_v<
                         std::remove_pointer_t<decltype(std::declval<C &>().data())> (*)[], T (*)[]>>>
  constexpr Span(C &c) : data_(c.data()), size_(c.size()) {}
  template <class U, class = std::enable_if_t<std::is_convertible_v<U (*)[], T (*)[]>>>
  constexpr Span(Span<U> const &o) noexcept : data_(o.data()), size_(o.size()) {}
  constexpr T *data() const noexcept { return data_; }
  constexpr std::size_t size() const noexcept { return size_; }
  constexpr bool empty() const noexcept { return size_ == 0; }
  constexpr T *begin() const noexcept { return data_; }
  constexpr T *end() const noexcept { return data_ + size_; }
  constexpr T &operator[](std::size_t n) const { return data_[n]; }
};
} // namespace ara::core
