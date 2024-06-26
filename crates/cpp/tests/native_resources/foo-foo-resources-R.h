/* User class definition file, autogenerated once, then user modified
 * Updated versions of this file are generated into
 * foo-foo-resources-R.h.template.
 */
namespace foo {
namespace foo {
namespace resources {
class R : public wit::ResourceImportBase<R> {
  uint32_t value;

public:
  static void Dtor(R *self) { delete self; };
  R(uint32_t a) : value(a) {}
  static Owned New(uint32_t a) { return Owned(new R(a)); }
  void Add(uint32_t b) { value += b; }

  uint32_t GetValue() const { return value; }
};

} // namespace resources
} // namespace foo
} // namespace foo
