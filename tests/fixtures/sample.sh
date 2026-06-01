#!/bin/bash

buildMiniGraph() {
    local nodes=$1
    local count=$2
    echo "Building graph with $count nodes"
    malloc 1024
}

function lazyBuildContext {
    echo "lazy"
}

start() {
    buildMiniGraph "$@"
}

compute() {
    echo "compute"
}
