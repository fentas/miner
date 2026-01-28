cmake_minimum_required(VERSION 3.16)
project(btcminer VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Detect CPU features
include(CheckCXXCompilerFlag)

check_cxx_compiler_flag("-msha" HAVE_SHA_NI)
check_cxx_compiler_flag("-mavx2" HAVE_AVX2)
check_cxx_compiler_flag("-msse4.1" HAVE_SSE41)

# Optimization flags
set(CMAKE_CXX_FLAGS_RELEASE "-O3 -DNDEBUG")

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

# Source files
set(SOURCES
    src/main.cpp
    src/stratum.cpp
    src/miner.cpp
)

add_executable(btcminer ${SOURCES})

# Platform-specific settings
if(HAVE_SHA_NI)
    target_compile_options(btcminer PRIVATE -msha -msse4.1)
    target_compile_definitions(btcminer PRIVATE HAVE_SHA_NI)
    message(STATUS "SHA-NI support enabled")
endif()

if(HAVE_AVX2)
    target_compile_options(btcminer PRIVATE -mavx2)
    target_compile_definitions(btcminer PRIVATE HAVE_AVX2)
    message(STATUS "AVX2 support enabled")
endif()

if(HAVE_SSE41)
    target_compile_options(btcminer PRIVATE -msse4.1)
    target_compile_definitions(btcminer PRIVATE HAVE_SSE41)
    message(STATUS "SSE4.1 support enabled")
endif()

# Native build option for maximum performance
option(NATIVE_BUILD "Build optimized for local CPU" ON)
if(NATIVE_BUILD)
    target_compile_options(btcminer PRIVATE -march=native)
    message(STATUS "Native CPU optimization enabled")
endif()

# Threading
find_package(Threads REQUIRED)
target_link_libraries(btcminer PRIVATE Threads::Threads)

# Install
install(TARGETS btcminer RUNTIME DESTINATION bin)
