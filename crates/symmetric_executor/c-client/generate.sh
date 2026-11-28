#!/bin/sh
cd "$(dirname "$0")"
../../../target/debug/wit-bindgen c ../wit -w executor-import --symmetric
../../../target/debug/wit-bindgen c ../wit -w module --symmetric --with symmetric:runtime/symmetric-executor@0.4.0=executor_import.h
