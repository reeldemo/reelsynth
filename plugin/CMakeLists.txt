cmake_minimum_required(VERSION 3.22)
project(reelsynth_plugin_stub VERSION 0.1.0 LANGUAGES CXX)

# Phase 5 stub — JUCE not vendored yet.
message(STATUS "reelsynth plugin scaffold: JUCE integration pending")
add_custom_target(reelsynth_plugin_stub
    COMMAND ${CMAKE_COMMAND} -E echo "ReelSynth VST3/AU build not wired in v0.1"
)
