# Mityu's CPU instruction-set baseline for every CMake dependency we build from
# source. Delivered as CMAKE_TOOLCHAIN_FILE from the repo-root `.cargo/config.toml`,
# so a local build and a CI build agree. That divergence was the bug.
#
# WHY THIS FILE EXISTS
#
# v1.2.2 hard-crashed for a user the first time Record worked, with
# EXCEPTION_ILLEGAL_INSTRUCTION (0xC000001D) at mityu.exe+0x1BBE25E. The bytes
# there are `62 F1 FE 08 7B 44 24 09` = EVEX-encoded `vcvtusi2ss xmm0, xmm0,
# qword ptr [rsp+0x48]` — an AVX-512F instruction. The CPU was an AMD Ryzen 7
# 7435HS (Zen 3+), which has AVX2 but no AVX-512, so the instruction is
# architecturally undefined and Windows killed the process. No Rust panic, no
# [ERROR] line, no dialog: the process simply vanished.
#
# It was not ONNX Runtime. `ort` links a SHA-pinned prebuilt static library, so
# local and CI link byte-identical ORT. It was ggml, which `whisper-rs-sys`
# compiles from source. ggml defaults GGML_NATIVE to ON, and with it on,
# `ggml/src/CMakeLists.txt` includes `../cmake/FindSIMD.cmake`, which decides the
# `/arch:` flag with CheckCSourceRuns — it COMPILES AND EXECUTES an AVX-512 probe
# **on the build machine** — and then applies the answer image-wide with
# add_compile_options(), with no runtime dispatch anywhere.
#
# So the instruction set of a Mityu release was a property of whichever GitHub
# runner happened to build it. The runner's CPU passed the probe. The owner's
# does not. Proof that this is the mechanism, from the owner's own machine:
#
#     whisper-rs-sys-*/out/build/CMakeCache.txt
#       GGML_NATIVE:BOOL=ON
#       HAS_AVX512_1_COMPILED:INTERNAL=TRUE
#       HAS_AVX512_1_EXITCODE:INTERNAL=FAILED_TO_RUN   <-- the probe crashed here
#
# and the faulting 8-byte sequence occurs exactly once in each shipped binary
# (v1.2.1 and v1.2.2) and zero times in either locally built binary.
#
# Note v1.2.1 carries the same instruction. It never fired because v1.2.1 could
# not start a recording at all (see #58); v1.2.2 fixed those refusals and walked
# straight into a landmine that had been sitting there since the engine landed.
#
# WHAT THIS FILE DOES
#
# Pins the baseline to x86-64 + AVX + AVX2 + FMA — Haswell (2013) and Excavator
# (2015) onward — and forbids every extension that a consumer CPU may lack.
# GGML_NATIVE=OFF also flips ggml's own INS_ENB default to ON
# (ggml/CMakeLists.txt:88), which is what turns AVX/AVX2/FMA on as a declared
# baseline instead of a machine-dependent guess.
#
# The baseline is a PROMISE TO USERS, not an optimisation setting. Raising it
# breaks machines silently and invisibly, exactly as this crash did. If you ever
# raise it, publish the requirement and extend the runtime guard in
# `src-tauri/src/cpu.rs` in the same change.
#
# `option()` never overwrites an existing cache entry, so setting these here —
# before ggml's CMakeLists runs — is what makes them stick.

set(GGML_NATIVE      OFF CACHE BOOL "Mityu: never derive the ISA from the build machine" FORCE)

# The declared baseline.
set(GGML_AVX         ON  CACHE BOOL "Mityu CPU baseline" FORCE)
set(GGML_AVX2        ON  CACHE BOOL "Mityu CPU baseline" FORCE)
set(GGML_FMA         ON  CACHE BOOL "Mityu CPU baseline" FORCE)

# Everything above the baseline stays off. MSVC's /arch:AVX512 is all-or-nothing
# and has no runtime dispatch, which is what made this fatal rather than slow.
set(GGML_AVX512      OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AVX512_VBMI OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AVX512_VNNI OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AVX512_BF16 OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AVX_VNNI    OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AMX         OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AMX_TILE    OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AMX_INT8    OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
set(GGML_AMX_BF16    OFF CACHE BOOL "Mityu CPU baseline: above the floor" FORCE)
