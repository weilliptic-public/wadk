cmake_minimum_required(VERSION 3.10)
project(weilsdk)

# Specify the version of the library
set(WEILSDK_VERSION_MAJOR 1)
set(WEILSDK_VERSION_MINOR 0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Define the source files for the SDK
set(WEILSDK_SOURCES
    src/runtime.cpp
    src/memory.cpp
    src/error.cpp
    src/ledger.cpp
    src/weil_contracts/fungible.cpp
    src/weil_contracts/nonFungible.cpp
)

# Set the output directories for the compiled library
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)  # For static libraries
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)  # For shared libraries
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)  # For executables

# Define where to find the headers
include_directories(${PROJECT_SOURCE_DIR}/include)

# Add the library target (static and shared)
# add_library(weilsdk_static STATIC src/runtime.cpp)
# add_library(weilsdk_shared SHARED src/runtime.cpp)
add_library(weilsdk_static STATIC ${WEILSDK_SOURCES})
add_library(weilsdk_shared SHARED ${WEILSDK_SOURCES})

# Specify versioning for shared library

set_target_properties(weilsdk_shared PROPERTIES
    OUTPUT_NAME "weilsdk"
    CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -s STANDALONE_WASM -O3 -Wl,--allow-undefined --no-entry"
)

# Export header files for installation
install(TARGETS weilsdk_static weilsdk_shared
        LIBRARY DESTINATION lib
        ARCHIVE DESTINATION lib)

install(DIRECTORY include/weilsdk DESTINATION include)

