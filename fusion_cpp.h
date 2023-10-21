// Generated by `wit-bindgen` 0.1.0. DO NOT EDIT!
#ifndef __CPP_GUEST_BINDINGS_FUSION_H
#define __CPP_GUEST_BINDINGS_FUSION_H
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>
#include <expected>
#include <optional>
#include <cstring>
namespace fusion {  class ResourceBase{
    protected:
    int32_t handle;
    bool owned;
    public:
    ResourceBase(int32_t h=0, bool o=false) : handle(h), owned(o) {}
    int32_t get_handle() const { return handle; }
  }; }
namespace autosar { namespace ara { namespace types {  using ErrorCodeType = int32_t; }}}
namespace autosar { namespace ara { namespace types { class ErrorDomain  : fusion::ResourceBase {
  public:
  std::string Name();
  std::string Message(::autosar::ara::types::ErrorCodeType n);
//  int32_t wasm_handle() const {return handle;}
  ~ErrorDomain();
  ErrorDomain(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace types { 
  struct ErrorCode {
    ::autosar::ara::types::ErrorCodeType value;
    types::ErrorDomain  domain;
  };
}}}
namespace autosar { namespace ara { namespace e2exf { 
  enum class ConfigurationFormat : uint8_t {
    kJson = 0,
    kXml = 1,
  };
}}}
namespace autosar { namespace ara { namespace core {  using ErrorCode = ::autosar::ara::types::ErrorCode; }}}
namespace autosar { namespace ara { namespace com {  using ConfigurationFormat = ::autosar::ara::e2exf::ConfigurationFormat; }}}
namespace autosar { namespace ara { namespace core { class InstanceSpecifier  : fusion::ResourceBase {
  public:
  InstanceSpecifier(std::string_view const& spec);
  std::string ToString();
  InstanceSpecifier Clone();
  ~InstanceSpecifier();
  InstanceSpecifier(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace com {  using InstanceSpecifier = ::autosar::ara::core::InstanceSpecifier; }}}
namespace autosar { namespace ara { namespace exec { 
  enum class ExecutionState : uint8_t {
    kRunning = 0,
  };
}}}
// A "pollable" handle.
// 
// This is conceptually represents a `stream<_, _>`, or in other words,
// a stream that one can wait on, repeatedly, but which does not itself
// produce any data. It's temporary scaffolding until component-model's
// async features are ready.
// 
// And at present, it is a `u32` instead of being an actual handle, until
// the wit-bindgen implementation of handles and resources is ready.
// 
// `pollable` lifetimes are not automatically managed. Users must ensure
// that they do not outlive the resource they reference.
// 
// This [represents a resource](https://github.com/WebAssembly/WASI/blob/main/docs/WitInWasi.md#Resources).
namespace autosar { namespace ara { namespace poll {  using Pollable = uint32_t; }}}
namespace autosar { namespace ara { namespace radar {  using ErrorCode = ::autosar::ara::types::ErrorCode; }}}
namespace autosar { namespace ara { namespace radar {  using InstanceSpecifier = ::autosar::ara::core::InstanceSpecifier; }}}
namespace autosar { namespace ara { namespace radar {  using Pollable = ::autosar::ara::poll::Pollable; }}}
namespace autosar { namespace ara { namespace radar { 
  struct Position {
    int32_t x;
    int32_t y;
    int32_t z;
  };
}}}
namespace autosar { namespace ara { namespace radar { 
  struct RadarObjects {
    bool active;
    std::vector<uint8_t> object_vector;
  };
}}}
namespace autosar { namespace ara { namespace radar { 
  struct AdjustOutput {
    bool success;
    ::autosar::ara::radar::Position effective_position;
  };
}}}
namespace autosar { namespace ara { namespace radar { 
  enum class FusionVariant : uint8_t {
    kChina = 0,
    kUsa = 1,
    kEurope = 2,
    kRussia = 3,
  };
}}}
namespace autosar { namespace ara { namespace radar { 
  struct CalibrateOutput {
    bool call_result;
  };
}}}
namespace autosar { namespace ara { namespace radar { class FutureResultAdjustOutputErrorCode  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::expected<AdjustOutput, ErrorCode> Value();
  ~FutureResultAdjustOutputErrorCode();
  FutureResultAdjustOutputErrorCode(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class FutureResultCalibrateOutputErrorCode  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::expected<CalibrateOutput, ErrorCode> Value();
  ~FutureResultCalibrateOutputErrorCode();
  FutureResultCalibrateOutputErrorCode(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class FutureU32  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  uint32_t Value();
  ~FutureU32();
  FutureU32(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class StreamU32  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::optional<uint32_t> Value();
  ~StreamU32();
  StreamU32(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class FutureU16  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  uint16_t Value();
  ~FutureU16();
  FutureU16(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class StreamU16  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::optional<uint16_t> Value();
  ~StreamU16();
  StreamU16(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class ProxyHandle  : fusion::ResourceBase {
  public:
}; }}}
namespace autosar { namespace ara { namespace radar { class StreamProxyHandle  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::optional<radar::ProxyHandle > Value();
  ~StreamProxyHandle();
  StreamProxyHandle(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class StreamRadarObjects  : fusion::ResourceBase {
  public:
  ::autosar::ara::radar::Pollable Subscribe();
  std::optional<RadarObjects> Value();
  ~StreamRadarObjects();
  StreamRadarObjects(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace radar { class Proxy  : fusion::ResourceBase {
  public:
  Proxy(radar::ProxyHandle  handle);
  FutureResultAdjustOutputErrorCode Adjust(::autosar::ara::radar::Position const& target_position);
  FutureResultCalibrateOutputErrorCode Calibrate(std::string_view const& configuration, ::autosar::ara::radar::FusionVariant radar_variant);
  void Echo(std::string_view const& text);
  std::expected<radar::StreamRadarObjects , ErrorCode> SubscribeBrakeEvent(uint32_t max_sample_count);
  std::expected<radar::StreamRadarObjects , ErrorCode> SubscribeParkingBrakeEvent(uint32_t max_sample_count);
  FutureU32 GetUpdateRate();
  FutureU32 SetUpdateRate(uint32_t value);
  std::expected<radar::StreamU32 , ErrorCode> SubscribeUpdateRate(uint32_t max_sample_count);
  std::expected<radar::StreamU16 , ErrorCode> SubscribeFrontObjectDistance(uint32_t max_sample_count);
  FutureU16 GetRearObjectDistance();
  FutureU16 SetObjectDetectionLimit(uint16_t value);
  ~Proxy();
  Proxy(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace log { class Logger  : fusion::ResourceBase {
  public:
  void Report(uint32_t level, std::string_view const& message);
  Logger(std::string_view const& context, std::string_view const& description, uint32_t level);
  ~Logger();
  Logger(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace com { class InstanceIdentifier  : fusion::ResourceBase {
  public:
  InstanceIdentifier(std::string_view const& id);
  std::string ToString();
  ~InstanceIdentifier();
  InstanceIdentifier(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace exec { class ExecutionClient  : fusion::ResourceBase {
  public:
  ExecutionClient(void);
  void ReportExecutionState(::autosar::ara::exec::ExecutionState state);
  ~ExecutionClient();
  ExecutionClient(fusion::ResourceBase&&);
}; }}}
namespace autosar { namespace ara { namespace types { 
}}}
namespace autosar { namespace ara { namespace core { 
  std::expected<void, ErrorCode> Initialize(void);
  std::expected<void, ErrorCode> Deinitialize(void);
  std::expected<core::InstanceSpecifier , ErrorCode> CreateInstanceSpecifier(std::string_view const& spec);
}}}
namespace autosar { namespace ara { namespace poll { 
  // Dispose of the specified `pollable`, after which it may no longer
  // be used.
  void DropPollable(::autosar::ara::poll::Pollable ths);
  // Poll for completion on a set of pollables.
  // 
  // The "oneoff" in the name refers to the fact that this function must do a
  // linear scan through the entire list of subscriptions, which may be
  // inefficient if the number is large and the same subscriptions are used
  // many times. In the future, this is expected to be obsoleted by the
  // component model async proposal, which will include a scalable waiting
  // facility.
  // 
  // The result list<bool> is the same length as the argument
  // list<pollable>, and indicates the readiness of each corresponding
  // element in that / list, with true indicating ready.
  std::vector<bool> PollOneoff(std::vector<Pollable> const& in);
}}}
namespace autosar { namespace ara { namespace radar { 
  StreamProxyHandle StartFindService(radar::InstanceSpecifier  spec);
}}}
namespace autosar { namespace ara { namespace log { 
}}}
namespace autosar { namespace ara { namespace e2exf { 
}}}
namespace autosar { namespace ara { namespace com { 
  bool StatusHandlerConfigure(std::string_view const& binding_configuration, ::autosar::ara::com::ConfigurationFormat binding_format, std::string_view const& e2exf_configuration, ::autosar::ara::com::ConfigurationFormat e2exf_format);
  std::vector<com::InstanceIdentifier > ResolveInstanceIds(com::InstanceSpecifier  spec);
}}}
namespace autosar { namespace ara { namespace exec { 
}}}

#endif