// cargo passes the link search paths of dependency build scripts on to the final link, so
// every crate using the symmetric mode finds the runtime cdylibs
fn main() {
    if std::env::var_os("CARGO_FEATURE_SYMMETRIC").is_some() {
        symmetric_link_search::emit();
    }
}
