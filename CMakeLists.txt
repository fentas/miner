cmake_minimum_required(VERSION 3.16)
project(btcminer VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Get git hash
execute_process(
    COMMAND git rev-parse --short HEAD
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    OUTPUT_VARIABLE GIT_HASH
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET
)

if(NOT GIT_HASH)
    set(GIT_HASH "unknown")
endif()

# Build type string
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    set(BUILD_TYPE "release")
else()
    set(BUILD_TYPE "dev")
endif()

message(STATUS "Version: ${PROJECT_VERSION} (${GIT_HASH})")

# Detect CPU features
include(CheckCXXCompilerFlag)

check_cxx_compiler_flag("-msha" HAVE_SHA_NI)
check_cxx_compiler_flag("-mavx2" HAVE_AVX2)
check_cxx_compiler_flag("-mavx512f" HAVE_AVX512)
check_cxx_compiler_flag("-msse4.1" HAVE_SSE41)

# Optimization flags
set(CMAKE_CXX_FLAGS_RELEASE "-O3 -DNDEBUG")

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

# Options
option(NATIVE_BUILD "Build optimized for local CPU" ON)
option(ENABLE_CUDA "Enable CUDA GPU mining" OFF)
option(ENABLE_OPENCL "Enable OpenCL GPU mining" OFF)
option(BUILD_TESTS "Build tests" ON)
option(ENABLE_COVERAGE "Enable code coverage" OFF)

# Source files
set(SOURCES
    src/main.cpp
    src/stratum.cpp
    src/miner.cpp
    src/stratum_v2.cpp
)

# GPU sources
if(ENABLE_CUDA OR ENABLE_OPENCL)
    list(APPEND SOURCES src/gpu/gpu.cpp)
endif()

add_executable(btcminer ${SOURCES})

# Version info
target_compile_definitions(btcminer PRIVATE
    BTCMINER_GIT_HASH="${GIT_HASH}"
    BTCMINER_BUILD_TYPE="${BUILD_TYPE}"
)

# CPU feature flags
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

if(HAVE_AVX512)
    target_compile_options(btcminer PRIVATE -mavx512f -mavx512bw -mavx512dq)
    target_compile_definitions(btcminer PRIVATE HAVE_AVX512)
    message(STATUS "AVX512 support enabled")
endif()

if(HAVE_SSE41)
    target_compile_options(btcminer PRIVATE -msse4.1)
    target_compile_definitions(btcminer PRIVATE HAVE_SSE41)
    message(STATUS "SSE4.1 support enabled")
endif()

if(NATIVE_BUILD)
    target_compile_options(btcminer PRIVATE -march=native)
    message(STATUS "Native CPU optimization enabled")
endif()

# CUDA support
if(ENABLE_CUDA)
    enable_language(CUDA)
    find_package(CUDA REQUIRED)

    set(CUDA_SOURCES
        src/gpu/sha256d.cu
    )

    add_library(btcminer_cuda STATIC ${CUDA_SOURCES})
    set_target_properties(btcminer_cuda PROPERTIES
        CUDA_SEPARABLE_COMPILATION ON
        CUDA_ARCHITECTURES "60;70;75;80;86;89;90"
    )
    target_compile_definitions(btcminer_cuda PRIVATE HAVE_CUDA)

    target_link_libraries(btcminer PRIVATE btcminer_cuda ${CUDA_LIBRARIES})
    target_compile_definitions(btcminer PRIVATE HAVE_CUDA)
    target_include_directories(btcminer PRIVATE ${CUDA_INCLUDE_DIRS})

    message(STATUS "CUDA GPU mining enabled")
endif()

# OpenCL support
if(ENABLE_OPENCL)
    find_package(OpenCL REQUIRED)

    target_link_libraries(btcminer PRIVATE OpenCL::OpenCL)
    target_compile_definitions(btcminer PRIVATE HAVE_OPENCL)

    install(FILES src/gpu/sha256d.cl DESTINATION share/btcminer)

    message(STATUS "OpenCL GPU mining enabled")
endif()

# Threading
find_package(Threads REQUIRED)
target_link_libraries(btcminer PRIVATE Threads::Threads)

# Tests
if(BUILD_TESTS)
    set(TEST_SOURCES
        tests/test_main.cpp
        tests/test_sha256.cpp
        tests/test_sha256_avx.cpp
        tests/test_stratum.cpp
        tests/test_stratum_v2.cpp
        tests/test_gpu.cpp
        tests/test_config.cpp
        tests/test_connection.cpp
    )

    add_executable(btcminer_test ${TEST_SOURCES})

    # Apply CPU feature flags to tests
    if(HAVE_SHA_NI)
        target_compile_options(btcminer_test PRIVATE -msha -msse4.1)
        target_compile_definitions(btcminer_test PRIVATE HAVE_SHA_NI)
    endif()

    if(HAVE_AVX2)
        target_compile_options(btcminer_test PRIVATE -mavx2)
        target_compile_definitions(btcminer_test PRIVATE HAVE_AVX2)
    endif()

    if(HAVE_AVX512)
        target_compile_options(btcminer_test PRIVATE -mavx512f -mavx512bw -mavx512dq)
        target_compile_definitions(btcminer_test PRIVATE HAVE_AVX512)
    endif()

    if(NATIVE_BUILD)
        target_compile_options(btcminer_test PRIVATE -march=native)
    endif()

    # GPU definitions for tests
    if(ENABLE_CUDA)
        target_compile_definitions(btcminer_test PRIVATE HAVE_CUDA)
    endif()

    if(ENABLE_OPENCL)
        target_compile_definitions(btcminer_test PRIVATE HAVE_OPENCL)
    endif()

    target_link_libraries(btcminer_test PRIVATE Threads::Threads)

    # Coverage support
    if(ENABLE_COVERAGE)
        target_compile_options(btcminer_test PRIVATE --coverage -fprofile-arcs -ftest-coverage)
        target_link_options(btcminer_test PRIVATE --coverage)
        message(STATUS "Code coverage enabled")
    endif()

    enable_testing()
    add_test(NAME btcminer_test COMMAND btcminer_test)

    # Individual test targets
    add_test(NAME test_sha256 COMMAND btcminer_test --sha256)
    add_test(NAME test_avx COMMAND btcminer_test --avx)
    add_test(NAME test_stratum COMMAND btcminer_test --stratum)
    add_test(NAME test_stratum_v2 COMMAND btcminer_test --stratum-v2)
    add_test(NAME test_gpu COMMAND btcminer_test --gpu)
    add_test(NAME test_config COMMAND btcminer_test --config)
    add_test(NAME test_connection COMMAND btcminer_test --connection)

    message(STATUS "Tests enabled")
endif()

# Coverage target
if(ENABLE_COVERAGE AND BUILD_TESTS)
    find_program(LCOV lcov)
    find_program(GENHTML genhtml)

    if(LCOV AND GENHTML)
        add_custom_target(coverage
            COMMAND ${LCOV} --directory . --zerocounters
            COMMAND $<TARGET_FILE:btcminer_test>
            COMMAND ${LCOV} --directory . --capture --output-file coverage.info
            COMMAND ${LCOV} --remove coverage.info '/usr/*' '*/tests/*' --output-file coverage.info
            COMMAND ${GENHTML} coverage.info --output-directory coverage_html
            COMMAND ${CMAKE_COMMAND} -E echo "Coverage report: coverage_html/index.html"
            WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
            DEPENDS btcminer_test
            COMMENT "Generating code coverage report"
        )
        message(STATUS "Coverage target available: 'make coverage'")
    else()
        message(WARNING "lcov/genhtml not found, coverage report unavailable")
    endif()
endif()

# Install
install(TARGETS btcminer RUNTIME DESTINATION bin)
install(DIRECTORY web DESTINATION share/btcminer)
install(FILES btcminer.conf.example DESTINATION share/btcminer)
