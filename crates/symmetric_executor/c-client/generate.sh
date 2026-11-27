#!/bin/sh
cd "$(dirname "$0")"
../../../target/debug/wit-bindgen c ../wit -w executor-import --symmetric
../../../target/debug/wit-bindgen c ../wit -w module --symmetric --with symmetric:runtime/symmetric-executor@0.3.1=executor_import.h
