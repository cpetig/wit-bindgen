#pragma once
// STUB of ara::core::future_errc for compile tests of `--autosar`, see
// STUB_README.txt (the real header also defines FutureErrorDomain)
#include "error_code.h"
#include <cstdint>
namespace ara::core {
enum class future_errc : std::int32_t {
  broken_promise = 101,
  future_already_retrieved = 102,
  promise_already_satisfied = 103,
  no_state = 104,
};
constexpr ErrorCode MakeErrorCode(future_errc code, std::int64_t /*data*/) noexcept {
  return ErrorCode(static_cast<std::int64_t>(code));
}
} // namespace ara::core
