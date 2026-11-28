#include "module_cpp.h"
#include "stream_support.h"
#include <chrono>
#include <expected> // needs to precede optional
#include <future>
#include <memory>
#include <mutex>
#include <thread>
#include <type_traits>
#include <vector>

// internal calback used by lift_event
static inline symmetric::runtime::symmetric_executor::CallbackState
fulfil_promise_void(void *data) {
  std::unique_ptr<std::promise<void>> ptr((std::promise<void> *)data);
  ptr->set_value();
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

static inline std::future<void> lift_event(void *event) {
  std::promise<void> result;
  std::future<void> result1 = result.get_future();
  if (!event) {
    result.set_value();
  } else {
    std::unique_ptr<std::promise<void>> ptr =
        std::make_unique<std::promise<void>>(std::move(result));
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

// Background threads (std::async) which report to the executor, they are
// joined by an executor callback when finished. As nobody might call it before
// the library is unloaded (which would pull the code from under the thread)
// the remaining ones are joined then.
namespace wit {
namespace detail {
class BackgroundTasks {
  std::mutex lock_;
  std::vector<std::unique_ptr<std::future<void>>> tasks_;

public:
  std::future<void> *add(std::future<void> &&task) {
    auto owned = std::make_unique<std::future<void>>(std::move(task));
    std::future<void> *result = owned.get();
    std::lock_guard<std::mutex> guard(lock_);
    tasks_.push_back(std::move(owned));
    return result;
  }
  void join_and_remove(std::future<void> *task) {
    std::unique_ptr<std::future<void>> owned;
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
      owned->get();
  }
  ~BackgroundTasks() {
    std::vector<std::unique_ptr<std::future<void>>> remaining;
    {
      std::lock_guard<std::mutex> guard(lock_);
      remaining.swap(tasks_);
    }
    for (auto &task : remaining)
      task->get();
  }
};
inline BackgroundTasks background_tasks;
} // namespace detail
} // namespace wit

// internal callback for lower_async and the other background threads
static inline symmetric::runtime::symmetric_executor::CallbackState
wait_on_future(std::future<void> *fut) {
  wit::detail::background_tasks.join_and_remove(fut);
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

template <class T>
void *lower_async(std::future<T> &&result1,
                  std::function<void(T &&)> &&lower_result) {
  if (result1.wait_for(std::chrono::seconds::zero()) ==
      std::future_status::ready) {
    lower_result(result1.get());
    return nullptr;
  } else {
    // move to run_in_background
    symmetric::runtime::symmetric_executor::EventGenerator gen;
    auto waiting = gen.Subscribe();
    auto task = std::async(
        std::launch::async,
        [lower_result](
            std::future<wit::string> &&result1,
            symmetric::runtime::symmetric_executor::EventGenerator &&gen) {
          lower_result(result1.get());
          gen.Activate();
        },
        std::move(result1), std::move(gen));
    std::future<void> *fut = wit::detail::background_tasks.add(std::move(task));
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
  std::promise<T> promise;
  uint8_t value[LIFT::SIZE];
};

// internal: complete the promise of lift_future if the read has completed.
// Returns false if it is still pending.
template <class T, class LIFT>
static bool fulfil_promise_if_done(fulfil_promise_data<T, LIFT> &data) {
  auto buffer = data.stream.ReadResult();
  if (buffer.has_value()) {
    if constexpr (std::is_void_v<T>) {
      // future without payload
      data.promise.set_value();
    } else {
      data.promise.set_value(LIFT::lift(data.value));
    }
    return true;
  }
  if (buffer.error() == symmetric::runtime::symmetric_stream::StreamState::kEof) {
    // the writer is gone without a value
    data.promise.set_exception(std::make_exception_ptr(
        std::future_error(std::future_errc::broken_promise)));
    return true;
  }
  return false;
}

// internal callback used by lift_future
template <class T, class LIFT>
static symmetric::runtime::symmetric_executor::CallbackState
fulfil_promise(void *data) {
  auto *ptr = (fulfil_promise_data<T, LIFT> *)data;
  if (!fulfil_promise_if_done<T, LIFT>(*ptr)) {
    // not the event of the completion, wait for the next one
    return symmetric::runtime::symmetric_executor::CallbackState::kPending;
  }
  delete ptr;
  return symmetric::runtime::symmetric_executor::CallbackState::kReady;
}

template <class T, class LIFT> std::future<T> lift_future(uint8_t *stream) {
  std::promise<T> promise;
  std::future<T> result = promise.get_future();
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
          1, LIFT::SIZE);
  data->stream.StartReading(std::move(buf));
  // a writer which was first has completed the read already
  if (fulfil_promise_if_done<T, LIFT>(*data)) {
    return result;
  }
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

private:
  // the size of an element in the stream, elements without payload have none
  static constexpr size_t element_size() {
    return std::is_same_v<T, wit::Void> ? 0 : wit::StreamProperties<T>::lowered_size;
  }

  // `data` lowered, `done` of the elements belong to the reader
  struct lowered {
    std::vector<uint8_t> mem;
    size_t count;
    size_t done = 0;

    explicit lowered(std::vector<T> &data)
        : mem(data.size() * wit::StreamProperties<T>::lowered_size),
          count(data.size()) {
      for (size_t i = 0; i < count; ++i) {
        wit::StreamProperties<T>::lower(
            std::move(data[i]),
            mem.data() + i * wit::StreamProperties<T>::lowered_size);
      }
      data.clear();
    }
    // the elements not taken by the reader are ours again
    std::vector<T> rest() {
      std::vector<T> result;
      for (; done < count; ++done) {
        result.push_back(wit::StreamProperties<T>::lift(
            mem.data() + done * wit::StreamProperties<T>::lowered_size));
      }
      return result;
    }
  };

  // offer the elements, completed at once if a reader is waiting and takes all
  bool offer(lowered &data) {
    symmetric::runtime::symmetric_stream::Buffer buffer(
        symmetric::runtime::symmetric_stream::Address(wit::ResourceImportBase(
            (wit::ResourceImportBase::handle_t)(data.mem.data() +
                                                data.done * wit::StreamProperties<T>::lowered_size))),
        data.count - data.done, element_size());
    buffer.SetSize(data.count - data.done);
    // a write in flight is not possible, as everything is done in one call
    return handle.OfferWrite(std::move(buffer)).has_value();
  }

  // take a completed write into account
  enum class progress { pending, taken, eof };
  progress collect(lowered &data) {
    auto result = handle.WriteResult();
    if (result.has_value()) {
      data.done += result->GetSize();
      return progress::taken;
    }
    return result.error() == symmetric::runtime::symmetric_stream::StreamState::kEof
               ? progress::eof
               : progress::pending;
  }

public:
  // non blocking write, returns remaining data
  std::vector<T> write_nb(std::vector<T> data) {
    if (data.empty()) {
      return data;
    }
    lowered mem(data);
    if (!offer(mem)) {
      abort();
    }
    if (collect(mem) == progress::pending) {
      // no reader is waiting, take the elements back (unless it came meanwhile)
      auto buffer = handle.CancelWrite();
      if (buffer.has_value()) {
        mem.done += buffer->GetSize();
      }
    }
    // no need to handle eof
    return mem.rest();
  }

  // blocks until all data is written or the reader is gone (the elements the
  // reader took before it went away are not dropped)
  void write(std::vector<T> &&data) {
    if (data.empty()) {
      return;
    }
    lowered mem(data);
    if (!offer(mem)) {
      abort();
    }
    // The runtime keeps the write pending until the readers have taken all
    // elements (it remembers the offset), or they are gone.
    for (;;) {
      auto subsc = handle.WriteReadySubscribe();
      subsc.Reset();
      progress state = collect(mem);
      if (state != progress::pending) {
        // the elements not taken (reader gone) are dropped
        mem.rest();
        return;
      }
      symmetric::runtime::symmetric_executor::BlockOn(std::move(subsc));
    }
  }
  // bool IsReadyToWrite() const {
  //     return handle.IsReadyToWrite();
  // }
  symmetric::runtime::symmetric_executor::EventSubscription
  WriteReadySubscribe() const {
    return handle.WriteReadySubscribe();
  }

  ~stream_writer() {}
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

template <class T> future_value_t<T> take_future_value(std::future<T> &fut) {
  if constexpr (std::is_void_v<T>) {
    fut.get();
    return no_payload{};
  } else {
    return fut.get();
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
  std::future<T> fut;
};

// internal: the value is copied into the stream, nobody has to wait for the
// reader (and the value is dropped if there is none)
template <class T, class LOWER>
static void write_future_value(write_to_future_data<T> &data) {
  auto result = take_future_value(data.fut);
  alignas(std::max_align_t) uint8_t lowered[LOWER::SIZE];
  lower_future_value<T, LOWER>(std::move(result), lowered);
  symmetric::runtime::symmetric_stream::Buffer buffer(
      symmetric::runtime::symmetric_stream::Address(wit::ResourceImportBase(
          (wit::ResourceImportBase::handle_t)lowered)),
      1, LOWER::SIZE);
  buffer.SetSize(1);
  data.wr.handle.WriteDetached(std::move(buffer));
}

// internal function used by lower_future
template <class T, class LOWER>
static void write_to_future(std::unique_ptr<write_to_future_data<T>> ptr) {
  // is future ready?
  if (ptr->fut.wait_for(std::chrono::seconds::zero()) ==
      std::future_status::ready) {
    write_future_value<T, LOWER>(*ptr);
  } else {
    // sadly there is no easier way to wait for a future in the background?
    // move to run_in_background
    symmetric::runtime::symmetric_executor::EventGenerator gen;
    auto waiting = gen.Subscribe();
    auto task = std::async(
        std::launch::async,
        [](std::unique_ptr<write_to_future_data<T>> &&ptr,
           symmetric::runtime::symmetric_executor::EventGenerator &&gen) {
          write_future_value<T, LOWER>(*ptr);
          gen.Activate();
        },
        std::move(ptr), std::move(gen));
    std::future<void> *fut = wit::detail::background_tasks.add(std::move(task));
    symmetric::runtime::symmetric_executor::Register(
        waiting.Dup(),
        symmetric::runtime::symmetric_executor::CallbackFunction(
            wit::ResourceImportBase((uint8_t *)wait_on_future)),
        symmetric::runtime::symmetric_executor::CallbackData(
            wit::ResourceImportBase((uint8_t *)fut)));
  }
}

template <class T, class LOWER> uint8_t *lower_future(std::future<T> &&f) {
  auto handles = create_wasi_future<T>();
  write_to_future<T, LOWER>(std::make_unique<write_to_future_data<T>>(
      write_to_future_data<T>{std::move(handles.first), std::move(f)}));
  return handles.second.handle.into_handle();
}

template <class T> uint8_t *lower_stream(wit::stream<T> &&f) {
  return f.handle.into_handle();
}

// Async functions without coroutines (`--async-api future`, C++17).
//
// An imported async function returns a std::future, an exported async
// function is implemented by returning a std::future. Pending calls are
// bridged to the executor by threads:
// * import: the call returned an event, a thread blocks on it (which also
//   runs the executor) and then lifts the results, so the future can be
//   waited for by any thread.
// * export: a thread waits for the user's future, writes the results and
//   activates an event for the caller.
// Only the std::future/std::promise/std::async names (and the thread
// functions) have to be exchanged to use a different future implementation.
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
std::future<R> import_future(uint8_t *event, LIFT &&lift) {
  if (!event) {
    std::promise<R> promise;
    std::future<R> result = promise.get_future();
    if constexpr (std::is_void_v<R>) {
      lift();
      promise.set_value();
    } else {
      promise.set_value(lift());
    }
    return result;
  }
  return std::async(std::launch::async,
                    [event, lift = std::forward<LIFT>(lift)]() mutable -> R {
                      wait_for_event(event);
                      return lift();
                    });
}

template <class R, class LOWER>
void call_lower(std::future<R> &fut, LOWER &lower) {
  if constexpr (std::is_void_v<R>) {
    fut.get();
    lower();
  } else {
    lower(fut.get());
  }
}

/// Result of an async export: if `fut` is ready the results are written
/// (by `lower`) and null is returned, else a thread waits for it, writes the
/// results and activates the returned event.
template <class R, class LOWER>
uint8_t *export_future(std::future<R> &&fut, LOWER &&lower) {
  if (fut.wait_for(std::chrono::seconds::zero()) == std::future_status::ready) {
    call_lower(fut, lower);
    return nullptr;
  }
  symmetric::runtime::symmetric_executor::EventGenerator gen;
  auto waiting = gen.Subscribe();
  auto task = std::async(
      std::launch::async,
      [lower = std::forward<LOWER>(lower)](
          std::future<R> &&fut,
          symmetric::runtime::symmetric_executor::EventGenerator &&gen) mutable {
        call_lower(fut, lower);
        gen.Activate();
      },
      std::move(fut), std::move(gen));
  // the executor joins the thread once the results are written
  std::future<void> *joiner = wit::detail::background_tasks.add(std::move(task));
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
