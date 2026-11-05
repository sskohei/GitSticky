cmake_minimum_required(VERSION 3.22)
project(stickynote_android_sodium C)
# Windows cannot execute libsodium's autoconf script. Use the same portable C
# implementation and capability definitions as upstream's build.zig, with NDK.
if(NOT ANDROID)
  message(FATAL_ERROR "This helper is only for the Android NDK")
endif()
file(TO_CMAKE_PATH "${SODIUM_SOURCE}" SODIUM_SOURCE)
file(GLOB_RECURSE SODIUM_SOURCES CONFIGURE_DEPENDS "${SODIUM_SOURCE}/src/libsodium/*.c")
configure_file("${SODIUM_SOURCE}/builds/msvc/version.h"
  "${SODIUM_SOURCE}/src/libsodium/include/sodium/version.h" COPYONLY)
add_library(libsodium STATIC ${SODIUM_SOURCES})
set_target_properties(libsodium PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_include_directories(libsodium PRIVATE "${SODIUM_SOURCE}/src/libsodium/include" "${SODIUM_SOURCE}/src/libsodium/include/sodium")
target_compile_options(libsodium PRIVATE -fvisibility=hidden -fno-strict-aliasing -fno-strict-overflow -fwrapv -flax-vector-conversions)
target_compile_definitions(libsodium PRIVATE
  SODIUM_STATIC=1 CONFIGURED=1 DEV_MODE=1 _GNU_SOURCE=1
  HAVE_ATOMIC_OPS=1 HAVE_C11_MEMORY_FENCES=1 HAVE_GCC_MEMORY_FENCES=1
  HAVE_INLINE_ASM=1 HAVE_INTTYPES_H=1 HAVE_STDINT_H=1
  NATIVE_LITTLE_ENDIAN=1 ASM_HIDE_SYMBOL=.hidden TLS=_Thread_local
  HAVE_CATCHABLE_ABRT=1 HAVE_CATCHABLE_SEGV=1 HAVE_CLOCK_GETTIME=1
  HAVE_GETPID=1 HAVE_MADVISE=1 HAVE_MLOCK=1 HAVE_MMAP=1 HAVE_MPROTECT=1
  HAVE_NANOSLEEP=1 HAVE_POSIX_MEMALIGN=1 HAVE_PTHREAD_PRIO_INHERIT=1
  HAVE_PTHREAD=1 HAVE_RAISE=1 HAVE_SYSCONF=1 HAVE_SYS_AUXV_H=1
  HAVE_SYS_MMAN_H=1 HAVE_SYS_PARAM_H=1 HAVE_SYS_RANDOM_H=1 HAVE_WEAK_SYMBOLS=1)
