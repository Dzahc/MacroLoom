#!/bin/bash

# Builds and runs the release application
npm.cmd --prefix app run tauri -- build --no-bundle &&
  ./app/src-tauri/target/release/macroloom.exe
