cmake_minimum_required(VERSION 3.10)
set(PROJECT_NAME "rust_lib_umbra")
project(${PROJECT_NAME} LANGUAGES C)

include("${CMAKE_CURRENT_LIST_DIR}/../umbra_core.cmake")
umbra_bundle_core(${PROJECT_NAME} _umbra_bundled)
set(rust_lib_umbra_bundled_libraries
  "${_umbra_bundled}"
  PARENT_SCOPE
)
