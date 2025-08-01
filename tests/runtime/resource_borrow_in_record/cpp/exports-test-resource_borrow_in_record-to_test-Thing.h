#pragma once
#include "wit.h"
#include <cstdint>
#include <map>
#include <utility>
#include <string_view>
/* User class definition file, autogenerated once, then user modified
 * Updated versions of this file are generated into Thing.template.
 */
namespace exports {
namespace test {
namespace resource_borrow_in_record {
namespace to_test {
class Thing : public wit::ResourceExportBase<Thing> {

public:
  static void Dtor(to_test::Thing *self) { delete self; }
  Thing(wit::string s) : contents(s.to_string()) {}
  static Owned New(wit::string s) { return Owned(new Thing(wit::string::from_view(std::string_view(s.to_string() + " new")))); }
  wit::string Get() const { return wit::string::from_view(std::string_view(contents + " get")); }
  static int32_t ResourceNew(to_test::Thing *self);
  static Thing *ResourceRep(int32_t id);
  static void ResourceDrop(int32_t id);

  // this is ugly but exactly how the Rust test is designed
  static Owned new_internal(wit::string s) { return Owned(new Thing(std::move(s))); }
  std::string const& get_internal() const { return contents; }

private:
  std::string contents;
};

} // namespace to_test
} // namespace resource_borrow_in_record
} // namespace test
} // namespace exports
