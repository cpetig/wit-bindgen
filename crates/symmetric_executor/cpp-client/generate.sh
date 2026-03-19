#!/bin/sh
../../../target/debug/wit-bindgen cpp ../wit -w executor-import --symmetric --api-style symmetric --format
../../../target/debug/wit-bindgen cpp ../wit -w module --symmetric --api-style symmetric --format --with symmetric:runtime/symmetric-executor@0.3.1=executor_import_cpp.h
