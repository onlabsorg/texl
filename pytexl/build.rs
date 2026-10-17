//! On macOS, lets the module link without libpython: the interpreter that loads
//! it provides the Python symbols. It does nothing on other platforms.

fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
