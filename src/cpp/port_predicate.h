#pragma once

#include <cstdint>
#include <functional>

namespace aibox::ffi {

struct PortPredicate {
    const std::function<bool(int)> *fn = nullptr;

    bool check(std::int32_t port) const {
        return !fn || !(*fn) || (*fn)(port);
    }
};

}  // namespace aibox::ffi
