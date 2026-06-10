cmake_minimum_required(VERSION 3.25)
project(dek_wfp_native LANGUAGES CXX)

add_library(dek_wfp_native STATIC src/dek_wfp_native.cpp)
target_include_directories(dek_wfp_native PUBLIC include)
target_compile_features(dek_wfp_native PRIVATE cxx_std_17)
target_link_libraries(dek_wfp_native PRIVATE Fwpuclnt Ws2_32)
