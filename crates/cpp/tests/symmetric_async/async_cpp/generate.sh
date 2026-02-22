#!/bin/sh
(cd src;../../../../../../target/debug/wit-bindgen cpp ../../wit/async_module.wit --symmetric --api-style symmetric)
