# TaskDep

This is a simple utility to display the graph of dependencies from [Task](https://taskfile.dev/).

## Installation

Clone this repo and run `cargo install --path .`.

## Usage

Run `taskdep` in the directory where you have a `Taskfile.yaml`. It will generate an SVG image file, and it will open your default web browser to display it.

Use `taskdep -s` to avoid launching a browser. Use `taskdep -h` for help.

The graph will display cycles in color Red.