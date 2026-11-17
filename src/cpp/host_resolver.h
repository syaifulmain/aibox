#pragma once

#include <functional>

#include "rust/cxx.h"

namespace aibox::ffi {

struct HostResolverBridge {
    const std::function<rust::Vec<rust::String>(rust::Str)> *fn = nullptr;

    rust::Vec<rust::String> resolve(rust::Str host) const {
        if (!fn || !(*fn))
            return {};
        return (*fn)(host);
    }
};

}  // namespace aibox::ffi
