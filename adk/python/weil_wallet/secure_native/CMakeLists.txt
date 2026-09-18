cmake_minimum_required(VERSION 3.16)
project(weil_secure_wallet VERSION 1.0.0 LANGUAGES C)

find_package(OpenSSL REQUIRED COMPONENTS Crypto)
find_package(CURL REQUIRED)

add_library(weil_secure_wallet SHARED weil_secure_wallet.c)
target_compile_features(weil_secure_wallet PRIVATE c_std_11)
target_include_directories(
    weil_secure_wallet
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
        $<INSTALL_INTERFACE:include>
)
target_link_libraries(weil_secure_wallet PRIVATE OpenSSL::Crypto CURL::libcurl)
set_target_properties(
    weil_secure_wallet
    PROPERTIES
        VERSION ${PROJECT_VERSION}
        SOVERSION 1
        C_VISIBILITY_PRESET hidden
)

include(GNUInstallDirs)
install(TARGETS weil_secure_wallet LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR})
install(FILES weil_secure_wallet.h DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
