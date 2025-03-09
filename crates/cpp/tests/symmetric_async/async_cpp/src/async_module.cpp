// Generated by `wit-bindgen` 0.3.0. DO NOT EDIT!

// Ensure that the *_component_type.o object is linked in
#ifdef __wasm32__
extern void __component_type_object_force_link_async_module(void);
void __component_type_object_force_link_async_module_public_use_in_this_compilation_unit(void) {
  __component_type_object_force_link_async_module();
}
#endif
#include "async_module_cpp.h"
#include "module_cpp.h"
#include <cstdlib> // realloc
#include <chrono>

extern "C" void *cabi_realloc(void *ptr, size_t old_size, size_t align, size_t new_size);

__attribute__((__weak__, __export_name__("cabi_realloc")))
void *cabi_realloc(void *ptr, size_t old_size, size_t align, size_t new_size) {
  (void) old_size;
  if (new_size == 0) return (void*) align;
  void *ret = realloc(ptr, new_size);
  if (!ret) abort();
  return ret;
}

static symmetric::runtime::symmetric_executor::CallbackState fulfil_promise(void* data) {
  std::unique_ptr<std::promise<void>> ptr((std::promise<void>*)data);
  ptr->set_value();
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

extern "C" void* testX3AtestX2FwaitX00X5BasyncX5Dsleep(int64_t);
std::future<void> test::test::wait::Sleep(uint64_t nanoseconds)
{
  std::promise<void> result;
  std::future<void> result1 = result.get_future();
  void* wait = testX3AtestX2FwaitX00X5BasyncX5Dsleep((int64_t(nanoseconds)));
  if (!wait) { result.set_value(); } else {
    std::unique_ptr<std::promise<void>> ptr = std::make_unique<std::promise<void>>(std::move(result));
    symmetric::runtime::symmetric_executor::EventSubscription ev = symmetric::runtime::symmetric_executor::EventSubscription(wit::ResourceImportBase((uint8_t*)wait));
    symmetric::runtime::symmetric_executor::CallbackFunction fun = symmetric::runtime::symmetric_executor::CallbackFunction(wit::ResourceImportBase((uint8_t*)fulfil_promise));
    symmetric::runtime::symmetric_executor::CallbackData data = symmetric::runtime::symmetric_executor::CallbackData(wit::ResourceImportBase((uint8_t*)ptr.release()));
    symmetric::runtime::symmetric_executor::Register(std::move(ev), std::move(fun), std::move(data));
  }
  return result1;
}

static symmetric::runtime::symmetric_executor::CallbackState wait_on_future(std::future<void>* fut) {
  fut->get();
  delete fut;
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

extern "C" 
void* testX3AtestX2Fstring_delayX00X5BasyncX5Dforward(uint8_t* arg0, size_t arg1, uint8_t* arg2)
{
  auto len0 = arg1;

  auto store = [arg2](wit::string && result1) {
    auto ptr2 = (uint8_t*)(result1.data());
    auto len2 = (size_t)(result1.size());
    result1.leak();

    *((size_t*)(arg2 + sizeof(void*))) = len2;
    *((uint8_t**)(arg2 + 0)) = ptr2;
  };

  auto result1 = exports::test::test::string_delay::Forward(std::string_view((char const*)(arg0), len0));
  if (result1.wait_for(std::chrono::seconds::zero()) == std::future_status::ready) {
    store(result1.get());
    return nullptr;
  } else {
    symmetric::runtime::symmetric_executor::EventGenerator gen;
    auto waiting = gen.Subscribe();
    auto task = std::async(std::launch::async, [store](std::future<wit::string>&& result1, 
            symmetric::runtime::symmetric_executor::EventGenerator &&gen){
      store(result1.get());
      gen.Activate();
    }, std::move(result1), std::move(gen));
    auto fut = std::make_unique<std::future<void>>(std::move(task));
    symmetric::runtime::symmetric_executor::Register(waiting.Dup(), 
      symmetric::runtime::symmetric_executor::CallbackFunction(wit::ResourceImportBase((uint8_t*)wait_on_future)),
      symmetric::runtime::symmetric_executor::CallbackData(wit::ResourceImportBase((uint8_t*)fut.release())));
    return waiting.into_handle();
  }
}

// Component Adapters
