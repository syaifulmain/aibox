#pragma once

#include <cstdint>
#include <functional>

#include "rust/cxx.h"

namespace aibox::ffi {

struct CommandRunnerBridge {
    const std::function<void(rust::Str, rust::Slice<const rust::String>, std::int32_t &,
                             rust::String &, rust::String &)> *fn = nullptr;

    void run(rust::Str program, rust::Slice<const rust::String> args, std::int32_t &exit_code,
             rust::String &std_out, rust::String &std_err) const {
        if (!fn || !(*fn)) {
            exit_code = -1;
            std_out = rust::String();
            std_err = rust::String();
            return;
        }
        (*fn)(program, args, exit_code, std_out, std_err);
    }
};

}  // namespace aibox::ffi
