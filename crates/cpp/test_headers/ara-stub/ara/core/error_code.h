#pragma once
// STUB of ara::core::ErrorCode for compile tests of `--autosar`, see STUB_README.txt
#include <cstdint>
namespace ara::core {
class ErrorCode {
  std::int64_t value_ = 0;

public:
  constexpr ErrorCode() = default;
  constexpr explicit ErrorCode(std::int64_t value) : value_(value) {}
  constexpr std::int64_t Value() const { return value_; }
  friend constexpr bool operator==(ErrorCode a, ErrorCode b) { return a.value_ == b.value_; }
};
} // namespace ara::core
