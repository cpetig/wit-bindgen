#include "module_cpp.h"
#include "stream_support.h"
#include "future_compat.h" // std::future or ara::core::Future
#include <chrono>
#include <memory>
#include <type_traits>
#include <vector>

// internal calback used by lift_event
static inline symmetric::runtime::symmetric_executor::CallbackState
fulfil_promise_void(void *data) {
  std::unique_ptr<wit::promise<void>> ptr((wit::promise<void> *)data);
  ptr->set_value();
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

static inline wit::future<void> lift_event(void *event) {
  wit::promise<void> result;
  wit::future<void> result1 = result.get_future();
  if (!event) {
    result.set_value();
  } else {
    std::unique_ptr<wit::promise<void>> ptr =
        std::make_unique<wit::promise<void>>(std::move(result));
    symmetric::runtime::symmetric_executor::EventSubscription ev =
        symmetric::runtime::symmetric_executor::EventSubscription(
            wit::ResourceImportBase((uint8_t *)event));
    symmetric::runtime::symmetric_executor::CallbackFunction fun =
        symmetric::runtime::symmetric_executor::CallbackFunction(
            wit::ResourceImportBase((uint8_t *)fulfil_promise_void));
    symmetric::runtime::symmetric_executor::CallbackData data =
        symmetric::runtime::symmetric_executor::CallbackData(
            wit::ResourceImportBase((uint8_t *)ptr.release()));
    symmetric::runtime::symmetric_executor::Register(
        std::move(ev), std::move(fun), std::move(data));
  }
  return result1;
}

// Background threads (wit::spawn) which report to the executor, they are
// joined by an executor callback when finished. As nobody might call it before
// the library is unloaded (which would pull the code from under the thread)
// the remaining ones are joined then.
namespace wit {
namespace detail {
class BackgroundTasks {
  std::mutex lock_;
  std::vector<std::unique_ptr<wit::task>> tasks_;

public:
  wit::task *add(std::unique_ptr<wit::task> &&owned) {
    wit::task *result = owned.get();
    std::lock_guard<std::mutex> guard(lock_);
    tasks_.push_back(std::move(owned));
    return result;
  }
  void join_and_remove(wit::task *task) {
    std::unique_ptr<wit::task> owned;
    {
      std::lock_guard<std::mutex> guard(lock_);
      for (auto it = tasks_.begin(); it != tasks_.end(); ++it) {
        if (it->get() == task) {
          owned = std::move(*it);
          tasks_.erase(it);
          break;
        }
      }
    }
    if (owned)
      owned->join();
  }
  ~BackgroundTasks() {
    std::vector<std::unique_ptr<wit::task>> remaining;
    {
      std::lock_guard<std::mutex> guard(lock_);
      remaining.swap(tasks_);
    }
    for (auto &task : remaining)
      task->join();
  }
};
inline BackgroundTasks background_tasks;
} // namespace detail
} // namespace wit

// internal callback for lower_async and the other background threads
static inline symmetric::runtime::symmetric_executor::CallbackState
wait_on_future(wit::task *fut) {
  wit::detail::background_tasks.join_and_remove(fut);
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

template <class T>
void *lower_async(wit::future<T> &&result1,
                  std::function<void(T &&)> &&lower_result) {
  if (wit::future_ready(result1)) {
    lower_result(wit::future_get(result1));
    return nullptr;
  } else {
    // move to run_in_background
    symmetric::runtime::symmetric_executor::EventGenerator gen;
    auto waiting = gen.Subscribe();
    auto task = wit::spawn(
        [lower_result](
            wit::future<wit::string> &&result1,
            symmetric::runtime::symmetric_executor::EventGenerator &&gen) {
          lower_result(wit::future_get(result1));
          gen.Activate();
        },
        std::move(result1), std::move(gen));
    wit::task *fut = wit::detail::background_tasks.add(std::move(task));
    symmetric::runtime::symmetric_executor::Register(
        waiting.Dup(),
        symmetric::runtime::symmetric_executor::CallbackFunction(
            wit::ResourceImportBase((uint8_t *)wait_on_future)),
        symmetric::runtime::symmetric_executor::CallbackData(
            wit::ResourceImportBase((uint8_t *)fut)));
    return waiting.into_handle();
  }
}

// internal data structure used by lift_future
template <class T, class LIFT> struct fulfil_promise_data {
  symmetric::runtime::symmetric_stream::StreamObj stream;
  wit::promise<T> promise;
  uint8_t value[LIFT::SIZE];
};

// internal callback used by lift_future
template <class T, class LIFT>
static symmetric::runtime::symmetric_executor::CallbackState
fulfil_promise(void *data) {
  std::unique_ptr<fulfil_promise_data<T, LIFT>> ptr(
      (fulfil_promise_data<T, LIFT> *)data);
  auto buffer = ptr->stream.ReadResult();
  if constexpr (std::is_void_v<T>) {
    // future without payload
    ptr->promise.set_value();
  } else {
    ptr->promise.set_value(LIFT::lift(ptr->value));
  }
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

template <class T, class LIFT> wit::future<T> lift_future(uint8_t *stream) {
  wit::promise<T> promise;
  wit::future<T> result = promise.get_future();
  auto stream2 = symmetric::runtime::symmetric_stream::StreamObj(
      wit::ResourceImportBase(stream));
  auto event = stream2.ReadReadySubscribe();
  std::unique_ptr<fulfil_promise_data<T, LIFT>> data =
      std::make_unique<fulfil_promise_data<T, LIFT>>(
          fulfil_promise_data<T, LIFT>{
              std::move(stream2), std::move(promise), {0}});
  symmetric::runtime::symmetric_stream::Buffer buf =
      symmetric::runtime::symmetric_stream::Buffer(
          symmetric::runtime::symmetric_stream::Address(wit::ResourceImportBase(
              (wit::ResourceImportBase::handle_t)&data->value)),
          1);
  data->stream.StartReading(std::move(buf));
  // return value doesn't influence callback logic
  symmetric::runtime::symmetric_executor::Register(
      std::move(event),
      symmetric::runtime::symmetric_executor::CallbackFunction(
          wit::ResourceImportBase((uint8_t *)&fulfil_promise<T, LIFT>)),
      symmetric::runtime::symmetric_executor::CallbackData(
          wit::ResourceImportBase((uint8_t *)data.release())));
  return result;
}

template <class T> wit::stream<T> lift_stream(uint8_t *stream) {
  return wit::stream<T>{symmetric::runtime::symmetric_stream::StreamObj(
      wit::ResourceImportBase((wit::ResourceImportBase::handle_t)stream))};
}

template <class T> struct future_writer {
  symmetric::runtime::symmetric_stream::StreamObj handle;
};

template <class T> struct future_reader {
  symmetric::runtime::symmetric_stream::StreamObj handle;
};

template <class T>
std::pair<future_writer<T>, future_reader<T>> create_wasi_future() {
  auto stream = symmetric::runtime::symmetric_stream::StreamObj();
  auto stream2 = stream.Clone(false);
  return std::make_pair<future_writer<T>, future_reader<T>>(
      future_writer<T>{std::move(stream)},
      future_reader<T>{std::move(stream2)});
}

template <class T> struct stream_writer {
  symmetric::runtime::symmetric_stream::StreamObj handle;

  // non blocking write, returns remaining data
  std::vector<T> write_nb(std::vector<T> data) {
    auto result = handle.StartWriting();
    if (wit::result_ok(result)) {
      auto buffer = wit::result_value(std::move(result));
      auto capacity = buffer.Capacity();
      uint8_t *dest = (uint8_t *)buffer.GetAddress().into_handle();
      auto elements = data.size();
      if (capacity < elements)
        elements = capacity;
      for (uint32_t i = 0; i < elements; ++i) {
        wit::StreamProperties<T>::lower(
            std::move(data[i]),
            dest + (i * wit::StreamProperties<T>::lowered_size));
      }
      buffer.SetSize(elements);
      handle.FinishWriting(std::move(buffer));

      if (capacity > data.size())
        capacity = data.size();
      data.erase(data.begin(), data.begin() + capacity);
    }
    // no need to handle eof or pending
    return data;
  }

  void write(std::vector<T> &&data) {
    while (!data.empty()) {
      auto subsc = handle.WriteReadySubscribe();
      subsc.Reset();
      data = write_nb(std::move(data));
      if (!data.empty()) {
        if (handle.IsReadClosed()) {
          return;
        }
        symmetric::runtime::symmetric_executor::BlockOn(std::move(subsc));
      }
    }
  }
  // bool IsReadyToWrite() const {
  //     return handle.IsReadyToWrite();
  // }
  symmetric::runtime::symmetric_executor::EventSubscription
  WriteReadySubscribe() const {
    return handle.WriteReadySubscribe();
  }

  ~stream_writer() {
    // if (handle.get_handle()!=wit::ResourceImportBase::invalid) {
    //     handle.FinishWriting(std::optional<symmetric::runtime::symmetric_stream::Buffer>());
    // }
  }
  stream_writer(symmetric::runtime::symmetric_stream::StreamObj &&h)
      : handle(std::move(h)) {}
  stream_writer(const stream_writer &) = delete;
  stream_writer &operator=(const stream_writer &) = delete;
  stream_writer(stream_writer &&) = default;
  stream_writer &operator=(stream_writer &&) = default;
};

template <class T>
std::pair<stream_writer<T>, wit::stream<T>> create_wasi_stream() {
  auto stream = symmetric::runtime::symmetric_stream::StreamObj();
  auto stream2 = stream.Clone(false);
  return std::make_pair<stream_writer<T>, wit::stream<T>>(
      stream_writer<T>{std::move(stream)}, wit::stream<T>{std::move(stream2)});
}

// internal: the value of a future, which has none for future<void>
struct no_payload {};
template <class T>
using future_value_t = std::conditional_t<std::is_void_v<T>, no_payload, T>;

template <class T> future_value_t<T> take_future_value(wit::future<T> &fut) {
  if constexpr (std::is_void_v<T>) {
    wit::future_get(fut);
    return no_payload{};
  } else {
    return wit::future_get(fut);
  }
}

template <class T, class LOWER>
void lower_future_value(future_value_t<T> &&value, uint8_t *dest) {
  if constexpr (std::is_void_v<T>) {
    LOWER::lower(dest);
  } else {
    LOWER::lower(std::move(value), dest);
  }
}

// internal struct used by lower_future
template <class T> struct write_to_future_data {
  future_writer<T> wr;
  wit::future<T> fut;
};

// internal function used by lower_future
template <class T, class LOWER>
static symmetric::runtime::symmetric_executor::CallbackState
write_to_future(void *data) {
  std::unique_ptr<write_to_future_data<T>> ptr((write_to_future_data<T> *)data);
  // is future ready?
  if (wit::future_ready(ptr->fut)) {
    auto result = take_future_value(ptr->fut);
    auto written = ptr->wr.handle.StartWriting();
    if (wit::result_ok(written)) {
      auto buffer = wit::result_value(std::move(written));
      assert(buffer.Capacity() == 1);
      uint8_t *dataptr = (uint8_t *)(buffer.GetAddress().into_handle());
      lower_future_value<T, LOWER>(std::move(result), dataptr);
      buffer.SetSize(1);
      ptr->wr.handle.FinishWriting(std::move(buffer));
    } else {
      if (wit::result_error(written) ==
          symmetric::runtime::symmetric_stream::StreamState::kPending) {
        symmetric::runtime::symmetric_executor::Register(
            ptr->wr.handle.WriteReadySubscribe(),
            symmetric::runtime::symmetric_executor::CallbackFunction(
                wit::ResourceImportBase((uint8_t *)write_to_future<T, LOWER>)),
            symmetric::runtime::symmetric_executor::CallbackData(
                wit::ResourceImportBase((uint8_t *)ptr.release())));
      }
    }
  } else {
    // sadly there is no easier way to wait for a future in the background?
    // move to run_in_background
    symmetric::runtime::symmetric_executor::EventGenerator gen;
    auto waiting = gen.Subscribe();
    auto task = wit::spawn(
        [](std::unique_ptr<write_to_future_data<T>> &&ptr,
           symmetric::runtime::symmetric_executor::EventGenerator &&gen) {
          auto result = take_future_value(ptr->fut);
          while (!ptr->wr.handle.IsReadClosed()) {
            auto written = ptr->wr.handle.StartWriting();
            if (wit::result_ok(written)) {
              auto buffer = wit::result_value(std::move(written));
              // assert(buffer.GetSize()==1); //sizeof(T));
              uint8_t *dataptr =
                  (uint8_t *)(buffer.GetAddress().into_handle());
              lower_future_value<T, LOWER>(std::move(result), dataptr);
              buffer.SetSize(1);
              ptr->wr.handle.FinishWriting(std::move(buffer));
              // a future is written only once
              break;
            } else {
              if (wit::result_error(written) ==
                  symmetric::runtime::symmetric_stream::StreamState::kPending) {
                symmetric::runtime::symmetric_executor::BlockOn(
                    ptr->wr.handle.WriteReadySubscribe());
              }
            }
          }
          gen.Activate();
        },
        std::move(ptr), std::move(gen));
    wit::task *fut = wit::detail::background_tasks.add(std::move(task));
    symmetric::runtime::symmetric_executor::Register(
        waiting.Dup(),
        symmetric::runtime::symmetric_executor::CallbackFunction(
            wit::ResourceImportBase((uint8_t *)wait_on_future)),
        symmetric::runtime::symmetric_executor::CallbackData(
            wit::ResourceImportBase((uint8_t *)fut)));
  }
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

template <class T, class LOWER> uint8_t *lower_future(wit::future<T> &&f) {
  auto handles = create_wasi_future<T>();
  auto wait_on = handles.first.handle.WriteReadySubscribe();
  auto fut = std::make_unique<write_to_future_data<T>>(
      write_to_future_data<T>{std::move(handles.first), std::move(f)});
  symmetric::runtime::symmetric_executor::Register(
      std::move(wait_on),
      symmetric::runtime::symmetric_executor::CallbackFunction(
          wit::ResourceImportBase((uint8_t *)&write_to_future<T, LOWER>)),
      symmetric::runtime::symmetric_executor::CallbackData(
          wit::ResourceImportBase((uint8_t *)fut.release())));
  return handles.second.handle.into_handle();
}

template <class T> uint8_t *lower_stream(wit::stream<T> &&f) {
  return f.handle.into_handle();
}

// Async functions without coroutines (`--async-api future`, C++17).
//
// An imported async function returns a future, an exported async
// function is implemented by returning a future. Pending calls are
// bridged to the executor by threads:
// * import: the call returned an event, a thread blocks on it (which also
//   runs the executor) and then lifts the results, so the future can be
//   waited for by any thread.
// * export: a thread waits for the user's future, writes the results and
//   activates an event for the caller.
// The future, promise and thread names come from future_compat.h, which maps
// them to ara::core::Future/Promise for `--autosar` (WIT_ARA_FUTURE).
namespace wit {
namespace detail {

/// Storage for the results of an async import, it needs to stay valid until
/// the callee has completed
template <size_t Size, size_t Align> struct RetArea {
  alignas(Align) uint8_t data[Size];
};

/// Blocks (and runs the executor) until `event` is active, takes ownership.
inline void wait_for_event(uint8_t *event) {
  auto ev = symmetric::runtime::symmetric_executor::EventSubscription(
      wit::ResourceImportBase(event));
  for (;;) {
    symmetric::runtime::symmetric_executor::BlockOn(ev.Dup());
    if (ev.Ready())
      break;
    // block_on also returns if the executor has no work, but the event
    // can still be activated by another thread
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
}

/// Result of an async import: `event` is null if the call has completed,
/// else the future becomes ready once the event is active. `lift` reads the
/// results from the return area and is called after completion.
template <class R, class LIFT>
wit::future<R> import_future(uint8_t *event, LIFT &&lift) {
  if (!event) {
    wit::promise<R> promise;
    wit::future<R> result = promise.get_future();
    if constexpr (std::is_void_v<R>) {
      lift();
      promise.set_value();
    } else {
      promise.set_value(lift());
    }
    return result;
  }
  return wit::async_call<R>(
      [event, lift = std::forward<LIFT>(lift)]() mutable -> R {
        wait_for_event(event);
        return lift();
      });
}

template <class R, class LOWER>
void call_lower(wit::future<R> &fut, LOWER &lower) {
  if constexpr (std::is_void_v<R>) {
    wit::future_get(fut);
    lower();
  } else {
    lower(wit::future_get(fut));
  }
}

/// Result of an async export: if `fut` is ready the results are written
/// (by `lower`) and null is returned, else a thread waits for it, writes the
/// results and activates the returned event.
template <class R, class LOWER>
uint8_t *export_future(wit::future<R> &&fut, LOWER &&lower) {
  if (wit::future_ready(fut)) {
    call_lower(fut, lower);
    return nullptr;
  }
  symmetric::runtime::symmetric_executor::EventGenerator gen;
  auto waiting = gen.Subscribe();
  auto task = wit::spawn(
      [lower = std::forward<LOWER>(lower)](
          wit::future<R> &&fut,
          symmetric::runtime::symmetric_executor::EventGenerator &&gen) mutable {
        call_lower(fut, lower);
        gen.Activate();
      },
      std::move(fut), std::move(gen));
  // the executor joins the thread once the results are written
  wit::task *joiner = wit::detail::background_tasks.add(std::move(task));
  symmetric::runtime::symmetric_executor::Register(
      waiting.Dup(),
      symmetric::runtime::symmetric_executor::CallbackFunction(
          wit::ResourceImportBase((uint8_t *)wait_on_future)),
      symmetric::runtime::symmetric_executor::CallbackData(
          wit::ResourceImportBase((uint8_t *)joiner)));
  return waiting.into_handle();
}

} // namespace detail
} // namespace wit
