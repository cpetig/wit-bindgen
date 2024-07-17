// Generated by `wit-bindgen` 0.3.0. DO NOT EDIT!

// Ensure that the *_component_type.o object is linked in
#ifdef __wasm32__
extern void __component_type_object_force_link_a(void);
void __component_type_object_force_link_a_public_use_in_this_compilation_unit(
    void) {
  __component_type_object_force_link_a();
}
#endif
#include "a_cpp.h"
#include <cstdlib> // realloc

extern "C" void *cabi_realloc(void *ptr, size_t old_size, size_t align,
                              size_t new_size);

__attribute__((__weak__, __export_name__("cabi_realloc"))) void *
cabi_realloc(void *ptr, size_t old_size, size_t align, size_t new_size) {
  (void)old_size;
  if (new_size == 0)
    return (void *)align;
  void *ret = realloc(ptr, new_size);
  if (!ret)
    abort();
  return ret;
}

extern "C" __attribute__((import_module("foo:foo/resources")))
__attribute__((import_name("[resource-drop]r"))) void
fooX3AfooX2FresourcesX00X5Bresource_dropX5Dr(uint8_t *);
extern "C" __attribute__((import_module("foo:foo/resources")))
__attribute__((import_name("[constructor]r")))
uint8_t *fooX3AfooX2FresourcesX00X5BconstructorX5Dr(int32_t);
extern "C" __attribute__((import_module("foo:foo/resources")))
__attribute__((import_name("[method]r.add"))) void
fooX3AfooX2FresourcesX00X5BmethodX5DrX2Eadd(uint8_t *, int32_t);
extern "C" __attribute__((import_module("foo:foo/resources")))
__attribute__((import_name("create"))) uint8_t *
fooX3AfooX2FresourcesX00create();
extern "C" __attribute__((import_module("foo:foo/resources")))
__attribute__((import_name("consume"))) void
fooX3AfooX2FresourcesX00consume(uint8_t *);
foo::foo::resources::R::~R() {
  if (handle != nullptr) {
    fooX3AfooX2FresourcesX00X5Bresource_dropX5Dr(handle);
  }
}
foo::foo::resources::R::R(uint32_t a) {
  auto ret = fooX3AfooX2FresourcesX00X5BconstructorX5Dr((int32_t(a)));
  this->handle = wit::ResourceImportBase{ret}.into_handle();
}
void foo::foo::resources::R::Add(uint32_t b) const {
  fooX3AfooX2FresourcesX00X5BmethodX5DrX2Eadd((*this).get_handle(),
                                              (int32_t(b)));
}
foo::foo::resources::R::R(wit::ResourceImportBase &&b)
    : wit::ResourceImportBase(std::move(b)) {}
foo::foo::resources::R foo::foo::resources::Create() {
  auto ret = fooX3AfooX2FresourcesX00create();
  return wit::ResourceImportBase{ret};
}
void foo::foo::resources::Consume(R &&o) {
  fooX3AfooX2FresourcesX00consume(o.into_handle());
}

// Component Adapters
