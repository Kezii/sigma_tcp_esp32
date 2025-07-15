#!/bin/bash

watchexec -e rs -r -- "wasm-pack build --target web --no-typescript --no-pack --dev && cp pkg/* ../static/pkg/"