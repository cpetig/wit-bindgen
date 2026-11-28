#pragma once
#ifndef WIT_STREAM_SUPPORT_H
#define WIT_STREAM_SUPPORT_H

#include "module_cpp.h"
#include "result_compat.h"
#include <functional>

namespace wit {
/// Lifting (bytes to data), lowering and encoded byte size,
/// Specialize this struct for all types sent via streams
template <class T> struct StreamProperties {
  static const uint32_t lowered_size;
  static T lift(uint8_t const *);
  static void lower(T &&, uint8_t *);
};

template <class T> struct stream {
  symmetric::runtime::symmetric_stream::StreamObj handle;

  uint32_t buffer_size = 1;

  /// Construct this object with an invalid stream
  static stream<T> new_empty() {
    return stream<T>{symmetric::runtime::symmetric_stream::StreamObj(
                         wit::ResourceImportBase()),
                     1};
  }

private:
  struct background_object {
    symmetric::runtime::symmetric_stream::StreamObj handle;
    std::function<bool(wit::span<T>)> reader;
    std::vector<uint8_t> buffer;

    background_object(symmetric::runtime::symmetric_stream::StreamObj &&h,
                      std::function<bool(wit::span<T>)> &&r,
                      std::vector<uint8_t> &&b)
        : handle(std::move(h)), reader(std::move(r)), buffer(std::move(b)) {}
  };

  static symmetric::runtime::symmetric_executor::CallbackState
  data_available(background_object *data) {
    // several events (data and the end of the stream) can be merged into one
    // callback, so read until no data is left
    for (;;) {
      auto read = data->handle.ReadResult();
      if (!wit::result_ok(read)) {
        if (wit::result_error(read) ==
            symmetric::runtime::symmetric_stream::StreamState::kEof) {
          data->reader(wit::span<T>());
          auto release = std::unique_ptr<background_object>(data);
          return symmetric::runtime::symmetric_executor::CallbackState::kReady;
        }
        // just wait for the next event
        return symmetric::runtime::symmetric_executor::CallbackState::kPending;
      }
      auto buffer = wit::result_value(std::move(read));
      assert(buffer.GetAddress().into_handle() ==
             (wit::ResourceImportBase::handle_t)data->buffer.data());
      uint32_t size = buffer.GetSize();
      std::vector<T> lifted;
      lifted.reserve(size);
      for (uint32_t i = 0; i < size; ++i) {
        lifted.push_back(StreamProperties<T>::lift(
            data->buffer.data() + i * StreamProperties<T>::lowered_size));
      }
      if (size > 0 && !data->reader(wit::span<T>(lifted.data(), size))) {
        // the reader is not interested in more data, this closes the stream
        auto release = std::unique_ptr<background_object>(data);
        return symmetric::runtime::symmetric_executor::CallbackState::kReady;
      }

      auto res = data->handle.StartReading(std::move(buffer));
      if (!wit::result_ok(res)) {
        // Pending indicates that other reader was faster (misuse), handle as
        // eof for now
        data->reader(wit::span<T>());
        auto release = std::unique_ptr<background_object>(data);
        return symmetric::runtime::symmetric_executor::CallbackState::kReady;
      }
    }
  }

public:
  /// Amount of objects cached, builder like parametrization for set_reader
  stream<T> &buffering(uint32_t amount) {
    buffer_size = amount;
    return *this;
  }
  /// @brief Register a reader for data sent via the stream, until the end of
  /// the stream
  /// @return Handle to remove callback
  symmetric::runtime::symmetric_executor::CallbackRegistration
  set_reader(std::function<void(wit::span<T>)> &&fun) && {
    return std::move(*this).set_reader_until(
        [fun = std::move(fun)](wit::span<T> data) {
          fun(data);
          return true;
        });
  }

  /// @brief Register a reader for data sent via the stream, which stops
  /// reading (and closes the stream) when the function returns false
  /// @return Handle to remove callback
  symmetric::runtime::symmetric_executor::CallbackRegistration
  set_reader_until(std::function<bool(wit::span<T>)> &&fun) && {
    std::vector<uint8_t> buffer(buffer_size * StreamProperties<T>::lowered_size,
                                uint8_t(0));
    background_object *object =
        std::make_unique<background_object>(
            background_object{std::move(handle), std::move(fun),
                              std::move(buffer)})
            .release();

    symmetric::runtime::symmetric_stream::Buffer b(
        symmetric::runtime::symmetric_stream::Address(wit::ResourceImportBase{
            (wit::ResourceImportBase::handle_t)object->buffer.data()}),
        buffer_size);

    auto res = object->handle.StartReading(std::move(b));
    // success will trigger the event, a buffer indicates misuse
    assert(wit::result_ok(res));
    return symmetric::runtime::symmetric_executor::Register(
        object->handle.ReadReadySubscribe(),
        symmetric::runtime::symmetric_executor::CallbackFunction(
            wit::ResourceImportBase{
                (wit::ResourceImportBase::handle_t)&data_available}),
        symmetric::runtime::symmetric_executor::CallbackData(
            wit::ResourceImportBase{
                (wit::ResourceImportBase::handle_t)object}));
  }
  /// construct from external handle
  stream(symmetric::runtime::symmetric_stream::StreamObj &&h)
      : handle(std::move(h)) {}
  stream(const stream &) = delete;
  stream(stream &&) = default;
  stream &operator=(const stream &) = delete;
  stream &operator=(stream &&) = default;
};
} // namespace wit

#endif
