cmake_minimum_required(VERSION 3.10)
project(yutaka)

# Specify C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

set(LIBWEIL_DIR "${CMAKE_SOURCE_DIR}/../../../../adk/cpp/weilsdk/build/lib")

include_directories("${CMAKE_SOURCE_DIR}/../../../../adk/cpp/weilsdk/include")

add_executable(yutaka src/main.cpp)

target_link_libraries(yutaka "${LIBWEIL_DIR}/libweilsdk_static.a")

set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -s STANDALONE_WASM --no-entry -O3 -s ERROR_ON_UNDEFINED_SYMBOLS=0 -s INITIAL_MEMORY=10MB -s ALLOW_MEMORY_GROWTH=1")
